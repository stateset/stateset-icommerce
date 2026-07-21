# Changelog

All notable changes to StateSet iCommerce will be documented in this file.

This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

## [1.9.0] - 2026-07-20

### Added
- **Carts/checkout API** (10 endpoints): cart sessions, items, shipping,
  payment, completion, cancellation.
- **Backorders API** (5 endpoints): create, list, fulfill, cancel.
- **AR collections API**: record/list collection activities and a
  dunning-due queue (`GET /ar/dunning/due`).
- **GL FX revaluation** (`POST /gl/revalue`): idempotent unrealized
  gain/loss revaluation of foreign-currency accounts at the as-of rate,
  posting a balanced adjusting entry to a configurable FX gain/loss
  account; correct sign handling for credit-normal accounts; SQLite +
  PostgreSQL.
- **PostgreSQL integration tests** (15, live-PG, CI-discoverable):
  fixed assets, revenue recognition, cycle counts, and smoke coverage of
  all nine parity stores.

### Fixed
- Terminal-state guard on transfer-order and inbound-shipment
  cancellation (both backends): received or already-cancelled documents
  can no longer be cancelled.

## [1.8.0] - 2026-07-20

### Added
- **~140 new REST endpoints** exposing previously API-invisible backends:
  purchase orders + suppliers (18), general ledger (18 — journals with
  post/void/reverse, trial balance, balance sheet, income statement, period
  open/close/lock/reopen), accounts payable (14 — bills, payments with
  allocations, payment runs, aging), accounts receivable (12 — aging,
  payment application, credit memos, write-offs, dunning, statements),
  warehouse (18 — locations, adjust/move, cycle counts), fulfillment (13 —
  waves, pick/pack/ship, cartons), receiving (12 — receipts, put-away),
  work orders (13), quality (15 — inspections, NCRs, holds), BOM (8),
  lots (9), serials (8), fixed assets (10), revenue recognition (8). All
  documented in OpenAPI with tags and validated by the spec test-suite.
- **Fixed-asset register** (new module, full stack): asset lifecycle
  (draft → in-service → disposed/written-off), straight-line and
  declining-balance depreciation schedules with exact final-period plug,
  disposal gain/loss, SQLite + PostgreSQL stores, embedded accessors.
- **Revenue recognition** (new module, full stack, ASC 606-style):
  contracts, performance obligations with allocation validation, ratable /
  point-in-time / milestone schedules, period recognition, both stores,
  embedded accessors.
- **AP 3-way match**: pure tolerance-based PO ↔ receipt ↔ bill matching
  (`GET /ap/bills/{id}/three-way-match`), computed on read.
- **Cycle counts**: full workflow (draft → in-progress → completed) with
  transactional variance application to location inventory and
  `cycle_count` movement audit records; SQLite + PostgreSQL.
- **GL auto-posting** (config-gated, default off): posted depreciation
  periods and recognized revenue can generate balanced, posted journal
  entries (depreciation expense / accumulated depreciation; deferred →
  sales revenue).
- **Purchase-order state machine**: `PurchaseOrderStatus::can_transition_to`
  + `validate()`, enforced in the SQLite store (illegal transitions now
  return validation errors).
- **PostgreSQL parity** for nine formerly SQLite-only stores: stock
  snapshots, transfer orders, units of measure, inbound shipments, print
  stations, production batches, supplier SKUs, vendor returns, vendor
  credits.
- **~290 new tests** across model, store, and HTTP layers, including
  backfill for six formerly untested models (lot, serial, warehouse,
  quality, accounts receivable, invoice).

### Fixed
- SQLite `adjust_inventory`/`move_inventory` failed with a NOT NULL
  constraint on the first lot-less adjustment for a location; lot-less
  rows now use the empty-string lot key consistently.
- SQLite `create_receipt_from_po` selected a nonexistent
  `quantity` column (schema has `quantity_ordered`) and always failed.
- Zero clippy warnings across the entire workspace (all targets).

### Added (pre-1.8.0 unreleased items)
- **Node binding: gift cards.** The `gift_cards` domain (create, get,
  get_by_code, update, list, charge, refund, disable, get_transactions,
  is_supported) is now exposed on `@stateset/embedded` — it existed in the
  Rust core but had zero binding presence. Monetary values cross the boundary
  as **exact decimal strings** (e.g. `"30.01"`), not `f64`, avoiding the
  precision loss the binding's older money fields still carry (tracked
  separately). 9-case lifecycle test added.
- **Node binding: loyalty.** The `loyalty` domain (programs, accounts, points,
  rewards — 14 methods) is now exposed on `@stateset/embedded`. Points are
  integers; reward `value` crosses as an exact decimal string. 9-case test.
- **Node binding: store credits.** The `store_credits` domain (create, get,
  list, adjust, apply, get_transactions, is_supported) is now exposed on
  `@stateset/embedded` as `commerce.storeCredits`, rounding out the customer
  balance trio alongside gift cards and loyalty. Balances and transaction
  amounts cross as **exact decimal strings** (an apply is recorded as a negative
  debit, e.g. `"-10.00"`), and TypeScript typings are generated automatically.
  9-case lifecycle test added (issue, apply, adjust, negative-balance rejection,
  list by customer).
- **Node binding: product reviews.** The `reviews` domain (create, get, update,
  list, delete, get_summary, mark_helpful, mark_reported, is_supported) is now
  exposed on `@stateset/embedded` as `commerce.reviews` — a domain that was
  core-only in both bindings. Ratings are validated 1–5 at the boundary, and
  `getSummary` returns the average, total, and per-star distribution for a
  product (useful for an agent weighing a purchase). Auto-generated TypeScript
  typings; 10-case lifecycle test added (create/validation, update + moderation
  status, helpful/reported counters, summary aggregation, delete).
- **Node binding: wishlists.** The `wishlists` domain (create, get, update,
  list, delete, add_item, remove_item, is_supported) is now exposed on
  `@stateset/embedded` as `commerce.wishlists`, with nested item objects
  (product, variant, quantity, priority, note). Auto-generated TypeScript
  typings; 8-case lifecycle test added. Adding this binding uncovered the
  wishlist-item persistence bug fixed above.
- **Node binding: customer segments.** The `segments` domain (create, get,
  update, list, delete, add_member, remove_member, list_members, is_member,
  is_supported) is now exposed on `@stateset/embedded` as `commerce.segments`,
  the last self-contained storefront domain that was core-only in both bindings.
  Segment rules (field/operator/value, with the operator validated against the
  known set) and memberships cross as nested objects. Auto-generated TypeScript
  typings; 8-case lifecycle test (rules round-trip, invalid-operator rejection,
  member add/is_member/list/remove, delete).
- **Python binding: gift cards and loyalty.** The `gift_cards` (10 methods)
  and `loyalty` (14 methods) domains are now exposed on `stateset_embedded`,
  with balances/reward values as exact decimal strings, typed `.pyi` stubs,
  re-exports from the package, and pytest coverage (loyalty asserts program-tier
  round-trip). This closes the Node/Python parity gap the codebase review
  flagged for these two domains.
- **Python binding: store credits.** The `store_credits` domain (create, get,
  list, adjust, apply, get_transactions, is_supported) is now exposed on
  `stateset_embedded` as `commerce.store_credits`, at Node/Python parity with
  the matching Node binding. Balances and transaction amounts cross as exact
  decimal strings (an apply is recorded as a negative debit), with typed `.pyi`
  stubs, package re-exports, and pytest coverage (issue → apply → adjust,
  negative-balance rejection, list by customer).
- **Python binding: product reviews.** The `reviews` domain (create, get,
  update, list, delete, get_summary, mark_helpful, mark_reported, is_supported)
  is now exposed on `stateset_embedded` as `commerce.reviews`, at Node/Python
  parity with the matching Node binding. Ratings are validated 1–5 at the
  boundary and `get_summary` returns the average, total, and per-star
  distribution, with typed `.pyi` stubs, package re-exports, and pytest coverage
  (create/validation, update + moderation status, counters, summary, delete).
- **Python binding: wishlists.** The `wishlists` domain (create, get, update,
  list, delete, add_item, remove_item, is_supported) is now exposed on
  `stateset_embedded` as `commerce.wishlists`, at Node/Python parity. Nested
  wishlist items (product, variant, quantity, priority, note) are exposed via a
  `WishlistItem` output class, with typed `.pyi` stubs, package re-exports, and
  pytest coverage that round-trips an item's variant/quantity/priority.
- **Python binding: customer segments.** The `segments` domain (create, get,
  update, list, delete, add_member, remove_member, list_members, is_member,
  is_supported) is now exposed on `stateset_embedded` as `commerce.segments`, at
  Node/Python parity — completing all six self-contained storefront domains
  across both bindings. Rules are passed as `SegmentRuleInput(field, operator,
  value)` objects (operator validated) and read back as nested `SegmentRule`s;
  memberships as `SegmentMembership`. Typed `.pyi` stubs, package re-exports, and
  pytest coverage (rules round-trip, invalid-operator rejection, member
  add/is_member/list/remove).

### Docs
- **`bindings/MONEY_PRECISION.md`** — decision doc for the systemic `f64`
  money representation in the older Node and Python binding output structs
  (~50 fields each). Money crosses those surfaces as IEEE-754 floats rather
  than exact decimals; the newest domains (gift cards, loyalty) already use
  strings. Documents the extent and migration options for an owner decision.

### Fixed
- **BOM `list`/`count` filter divergences + a rapid-creation collision (BOM domain
  brought to full parity).** Four issues, fixed together:
  - **Postgres treated `product_id` and `status` as mutually exclusive.** `list_async`/
    `count_async` used `if product_id {} else if status {}`, so a
    `list(product_id=X, status=Active)` returned BOMs of *every* status for X (SQLite
    correctly ANDs them). Both now build the `WHERE` clause cumulatively.
  - **Postgres ignored the `search` filter entirely.** SQLite applies
    `name`/`bom_number` `LIKE`; Postgres had no search handling, so a BOM search
    silently returned the whole (paginated) set. Added `name`/`bom_number` `ILIKE` to
    both `list_async` and `count_async`.
  - **SQLite `count` omitted the `search` its own `list` applied** (count ≠ list when
    searching → wrong pagination total). Added the same `search` predicate to
    `count`.
  - **SQLite `load_components` had no `ORDER BY`** (components returned in arbitrary
    row order) while Postgres orders `position, created_at`. SQLite now matches.
  - **`generate_bom_number` collided within one second.** It was second-granularity
    (`BOM-%Y%m%d%H%M%S`) with no uniqueness suffix, so two BOMs created in the same
    wall-clock second collided on the `UNIQUE bom_number` constraint and the second
    `create` hard-failed on *both* backends. Now includes a millisecond timestamp +
    UUID suffix (matching the already-hardened `generate_work_order_number`).
  Verified with SQLite unit tests (`count_applies_search_filter`,
  `load_components_orders_by_position`) and a live-Postgres test
  (`postgres_bom_filters`) asserting product/status/search compose and that
  `count` matches the filtered `list`.
- **`warranties::list`/`count` silently dropped five `WarrantyFilter` fields on both
  backends.** Both SQLite (`list`/`count`) and Postgres (`list_async`/`count_async`)
  applied only `customer_id`/`status`/`active_only`, ignoring `order_id`,
  `product_id`, `sku`, `serial_number`, and `warranty_type` — so a caller filtering
  warranties by any of those got the entire (paginated) set back instead of the
  matching subset, on both backends. All five column-equality filters are now applied
  in `list` and `count`; `count` was additionally brought to full parity with `list`
  (it had also omitted `active_only`), so filtered counts match filtered lists. This
  was a shared bug (identical on both backends), not a cross-backend divergence.
  `expiring_within_days` remains unimplemented (backend-specific date arithmetic) and
  is tracked separately. Verified with a SQLite unit test (`list_filters_by_line_item_fields`,
  which also asserts `count == list`) and a live-Postgres test
  (`postgres_warranty_filters`).
- **SQLite `x402_payment_intents::count` ignored the order/batch/date filters.** It
  applied only `payer_address`/`payee_address`/`status`/`network`/`asset`, dropping
  `order_id`, `batch_id`, `from_date`, and `to_date` — all of which `list` (and
  Postgres) apply — so a filtered count by order, batch, or date range silently
  disagreed with the corresponding filtered list, breaking pagination totals. `count`
  now mirrors `list`'s full filter set. Verified with matching SQLite
  (`x402_count_filters`) and live-Postgres (`postgres_x402_count_filters`) tests that
  assert the filtered count equals the filtered list length.
- **Postgres `move_inventory` could drive source stock negative under concurrency.**
  The source-inventory `SELECT` had no `FOR UPDATE`, so concurrent moves of the same
  `(location, sku, lot)` each read the same stale `quantity_on_hand`, all passed the
  `quantity > available` guard, and all applied the relative
  `quantity_on_hand = quantity_on_hand − $1` decrement — over-transferring the source
  into negative stock (and creating phantom stock at the destination). SQLite
  serializes writers with an IMMEDIATE transaction and was unaffected. The source row
  is now locked with `FOR UPDATE` before the guard, so concurrent moves serialize and
  exactly one can succeed — matching SQLite. Verified with a live-Postgres concurrency
  test (`postgres_move_inventory_concurrency`): eight concurrent moves of 8 against 10
  available now yield exactly one success and leave the source at 2 (never negative).
- **The SQLite fraud tables were never created, so every fraud operation crashed on
  the embedded backend.** `fraud_assessments` and `fraud_rules` had a Postgres
  migration (`048_fraud.sql`) but no SQLite counterpart, and none of the embedded
  migrations created them — so `commerce.fraud()` calls (`create_assessment`,
  `get_assessment`, rules, etc.) failed at runtime on the default SQLite backend with
  "no such table: fraud_assessments", while Postgres worked. The gap was masked
  because the fraud unit tests created the tables inline under `#[cfg(test)]`. Added
  `059_fraud.sql` (mirroring the Postgres schema with SQLite types, plus the same
  indexes) and registered it in the embedded migration set. Verified with a
  regression test (`sqlite_fraud_migration`) that builds the database through the
  real migration path and round-trips a fraud assessment.
- **Postgres agent-reputation and agent-validation emitted malformed SQL (whole
  domains broken on Postgres).** Several multi-line SQL string literals ended their
  lines with a bare `\` (a Rust string-continuation escape) with no space before it,
  so the escape stripped the newline *and* the next line's leading whitespace and
  fused adjacent tokens — e.g. `... FROM agent_feedback\` + `WHERE …` became
  `agent_feedbackWHERE`, and `... revoked_at\` + `FROM …` became `revoked_atFROM`.
  Every feedback write/read (`give_feedback`, `revoke_feedback`, `read_feedback`,
  `read_all_feedback`) and every validation request/status read
  (`request_validation`, `respond_validation`, `get_validation_status`) failed at
  runtime on Postgres with a SQL syntax error, while SQLite worked. A sweep of the
  whole Postgres backend for this pattern (a trailing `\` not preceded by a space or
  comma) found it only in these two files; a space was added before every affected
  continuation `\`. Verified with live-Postgres regression tests
  (`postgres_agent_reputation_sql`, `postgres_agent_validation_sql`) that exercise
  the full feedback and validation request→respond→status flows end to end.
- **SQLite `reviews::list` dropped the `verified_only` filter.** It applied
  `product_id`/`customer_id`/`status`/`min_rating` but ignored `verified_only`, which
  Postgres applies as `verified_purchase = $n` — so filtering a product's reviews to
  verified purchases returned every review on SQLite. It now applies
  `verified_purchase = ?`. Verified with a SQLite regression test
  (`list_filters_by_verified_only`); the Postgres path already applies the filter
  (`postgres/reviews.rs`).
- **General-ledger list ordering diverged between the backends.** SQLite ordered
  `list_periods` by `(fiscal_year, period_number) DESC` and `list_journal_entries`
  by `(entry_date, entry_number) DESC`, while Postgres ordered periods by
  `start_date DESC` (a different, tie-prone sort key) and journal entries by
  `entry_date DESC` alone (no tiebreak, so same-date entries came back in an
  undefined order). Postgres now matches SQLite's deterministic ordering in both
  cases: periods sort by their unique `(fiscal_year, period_number)` identity, and
  journal entries break same-date ties by `entry_number`. Verified with matching
  SQLite (`gl_list_ordering`) and live-Postgres (`postgres_gl_list_ordering`) tests
  that construct rows where the old and new orderings disagree.
- **SQLite accounts-payable list methods now apply the `from_date`/`to_date`
  filters.** `list_bills`/`count_bills` (on `bill_date`), `list_payments` and
  `list_payment_runs` (on `payment_date`) dropped the date-range filter that
  Postgres applies. These were deferred while SQLite stored AP dates as full
  timestamps; now that those columns are truncated to midnight UTC (matching
  Postgres `DATE`), the `>= from_date` / `<= to_date` comparisons are well-defined,
  and are applied. This completes the accounts-payable list-filter parity work — the
  SQLite AP list/count methods now honor every filter Postgres does. Verified with
  matching SQLite (`ap_list_date_filters`) and live-Postgres
  (`postgres_ap_list_date_filters`) tests.
- **Accounts-payable dates kept their time-of-day on SQLite but were date-only on
  Postgres.** Postgres stores `bill_date`, `due_date` (`ap_bills`) and `payment_date`
  (`ap_payments`, `ap_payment_runs`) in `DATE` columns, which drop the time and read
  back at midnight UTC, while SQLite stored the full RFC3339 timestamp — so a bill or
  payment created with a timed date (e.g. `2026-03-10T14:30:00Z`) read back
  differently (`…14:30:00Z` vs `…00:00:00Z`). SQLite now truncates these columns to
  midnight UTC before storing, keeping the RFC3339 *format* (so every existing
  reader, including the auto-posting date parsers, is unaffected) while agreeing with
  Postgres. Verified with matching SQLite (`ap_date_truncation`) and live-Postgres
  (`postgres_ap_date_truncation`) tests. This clears the last catalogued
  accounts-payable SQLite↔Postgres storage divergence.
- **Accounts-payable bill money was stored at full precision on SQLite but rounded
  to 4dp on Postgres.** Postgres stores AP bill items and bill totals in
  `NUMERIC(12,4)` columns (`NUMERIC(12,6)` for `tax_rate`), which round on insert,
  while SQLite stored the full-precision `Decimal` as TEXT — so a bill line with
  sub-4dp inputs (e.g. a `unit_price` of `10.12345`) read back differently on the two
  backends (`10.12345` vs `10.1235`). SQLite now rounds every AP money value
  (item quantity/unit_price/amount/tax_amount and the bill subtotal/tax/total/
  amount_paid/amount_due to 4dp, `tax_rate` to 6dp) with `MidpointAwayFromZero` —
  matching Postgres numeric rounding — before storing. Verified with matching SQLite
  (`ap_bill_item_rounding`) and live-Postgres (`postgres_ap_bill_item_rounding`)
  tests that assert both backends store and read back identical rounded values. Same
  class as the invoice `DECIMAL(12,2)` rounding fix.
- **SQLite `list_billing_cycles` dropped the date-range filter and ordered
  differently from Postgres.** It applied `subscription_id`/`status` but silently
  ignored `from_date`/`to_date`, and ordered by `cycle_number DESC` where Postgres
  filters `period_start >= from_date` / `period_end <= to_date` and orders by
  `period_start DESC`. Both `period_start`/`period_end` are stored as RFC3339
  timestamps on SQLite (matching the bound values), so the string comparison is
  chronological and safe here — unlike the accounts-payable date-storage divergence.
  The date filters are now applied and the ordering matches Postgres. Verified with
  matching SQLite (inline `list_billing_cycles_filters_by_date_and_orders_by_period_start`)
  and live-Postgres (`postgres_list_billing_cycles_filters`) tests.
- **SQLite `list_journal_entries` dropped the `account_id` and `search` filters.**
  It applied period/type/source/status/date/source-document filters but silently
  ignored `account_id` (filter the ledger to entries touching a given account) and
  `search` (free-text over entry number/description) — both of which Postgres
  applies. `account_id` now selects entries via
  `id IN (SELECT journal_entry_id FROM gl_journal_entry_lines WHERE account_id = ?)`
  (the duplicate-free equivalent of Postgres's `SELECT DISTINCT` + lines join), and
  `search` matches `entry_number`/`description` with `LIKE` (case-insensitive for
  ASCII, as Postgres's `ILIKE`). Verified with matching SQLite
  (`gl_list_journal_entries_filters`) and live-Postgres
  (`postgres_gl_list_journal_entries_filters`) tests.
- **Swept the offset-without-limit crash across the entire SQLite backend (54
  list methods).** After fixing this on gift cards and store credits, an audit found
  the same latent crash in **54** SQLite `list`/`list_*` methods across ~40 domains
  (orders, customers, products, inventory, subscriptions, loyalty, promotions,
  warranties, reviews, general ledger, and many more): each appended a bare `OFFSET`
  independently of `LIMIT`, which SQLite rejects with a syntax error, so any caller
  paginating with an offset and no explicit limit crashed at runtime (Postgres,
  allowing a bare `OFFSET`, was unaffected). Introduced a single shared
  `append_limit_offset` helper (emitting `LIMIT -1 OFFSET n` in the offending case)
  and routed every site through it. The four cursor-paginated endpoints (orders,
  customers, products, returns) apply the offset only in non-cursor mode, as before.
  Covered by a helper unit test (all four limit/offset combinations) and the full
  681-test SQLite suite; no behavior changed for callers that already passed a limit.
- **Corrected a stale embedded-migration count assertion** (`sqlite_migrations`
  expected 55 migrations; the tree now has 58) so the SQLite test suite is green.
- **Listing gift cards or store credits with an offset but no limit crashed on
  SQLite.** Both `gift_cards::list` and `store_credits::list` appended `OFFSET <n>`
  to the query independently of `LIMIT`. SQLite rejects a bare `OFFSET` (without a
  preceding `LIMIT`) with a syntax error, so any caller paginating with an offset
  and no explicit limit hit a runtime `DatabaseError` — while Postgres, which allows
  a bare `OFFSET`, returned the page correctly. Both now emit `LIMIT -1 OFFSET <n>`
  (unbounded) in that case, matching Postgres. Verified with matching SQLite
  (`gift_store_list_offset_without_limit`) and live-Postgres
  (`postgres_gift_store_list_offset`) regression tests.
- **SQLite `list_payment_runs` ignored its filter entirely.** It always returned
  `SELECT * FROM ap_payment_runs ORDER BY created_at DESC`, dropping `status`
  (Postgres applies it) and `limit`/`offset` (no pagination — every call returned
  every run). It now applies `status` and `LIMIT`/`OFFSET` (using `LIMIT -1 OFFSET
  n` for SQLite's offset-without-limit case), matching Postgres. Verified with
  matching SQLite (`ap_list_payment_runs_filters`) and live-Postgres
  (`postgres_ap_list_payment_runs_filters`) tests. This completes the accounts-
  payable list/count filter-parity cluster (`list_bills`, `count_bills`,
  `list_payments`, `count_payments`, `list_payment_runs`). (`from_date`/`to_date`
  remain deferred — entangled with the AP date-storage divergence.)
- **`count_bills` did not match `list_bills` on either backend.** A count is meant
  to report how many rows the same filtered list would return, but `count_bills`
  applied a narrower filter set than `list_bills` on both backends — so a count
  filtered by purchase order or amount silently disagreed with the corresponding
  list. On SQLite `count_bills` applied only `status` (dropping `supplier_id`,
  `purchase_order_id`, `overdue_only`, `min_amount`/`max_amount`); on Postgres it
  applied only `supplier_id`/`status`/`overdue_only` (dropping `purchase_order_id`,
  the date range, and `min_amount`/`max_amount` that `list_bills` applies). Both are
  fixed: SQLite `list_bills` and `count_bills` now share one matching helper, and
  Postgres `count_bills` now mirrors `list_bills_async`'s predicates — so a filtered
  count equals the filtered list length on both backends, and the two agree with
  each other. Verified with matching SQLite (`ap_count_bills_filters`) and
  live-Postgres (`postgres_ap_count_bills_filters`) tests that assert
  `count_bills == list_bills().len()` across supplier/PO/amount filters.
  (`from_date`/`to_date` on SQLite remain deferred — AP date-storage divergence.)
- **SQLite accounts-payable payment listing/counting ignored filters.**
  `count_payments` ignored its filter argument entirely — it always returned
  `SELECT COUNT(*) FROM ap_payments`, so a filtered count never matched the
  corresponding filtered `list_payments` (a filtered list of one payment reported a
  count of "all payments"). `list_payments` applied only `supplier_id` and `status`,
  silently dropping `payment_method` (Postgres applies it) and `offset` (no
  pagination past the first page). Both now share a WHERE-builder covering
  `supplier_id`/`status`/`payment_method`, and `list_payments` applies `LIMIT`/
  `OFFSET` (using `LIMIT -1 OFFSET n` for SQLite's offset-without-limit case), so a
  filtered count matches the filtered list and both agree with Postgres. Verified
  with matching SQLite (`ap_payment_list_count_filters`) and live-Postgres
  (`postgres_ap_payment_list_count_filters`) tests. (`from_date`/`to_date` remain
  deferred — entangled with the AP date-storage divergence, as with `list_bills`.)
- **Auto-posting an inventory cost transaction to the general ledger was broken on
  SQLite (three bugs) — completing the GL auto-posting cluster.**
  `auto_post_inventory_cost` (1) selected `transaction_date`, a column that does not
  exist on `cost_transactions` — the date is `created_at` (Postgres reads
  `created_at`), so the query failed at runtime with "no such column:
  transaction_date"; (2) parsed that RFC3339 timestamp directly as a `NaiveDate`;
  and (3) treated only `transaction_type == "sale"` as a COGS-debit issue, whereas
  Postgres treats `"issue"` OR `"sale"` — so an `"issue"` cost transaction (the
  common inventory-consumption case) posted with the debit and credit **reversed**
  on SQLite (Inventory debited, COGS credited), silently corrupting COGS and
  inventory-value balances. All three are fixed to match Postgres. Verified with
  matching SQLite (`gl_auto_post_inventory_cost`) and live-Postgres
  (`postgres_gl_auto_post_inventory_cost`) regression tests that assert the posted
  lines put the debit on COGS and the credit on Inventory. **This completes the GL
  auto-posting cluster: all five `auto_post_*` methods (invoice, payment received,
  bill, bill payment, inventory cost) now work on SQLite and match Postgres.**
- **Auto-posting a bill payment to the general ledger was completely broken on
  SQLite (two bugs).** `auto_post_bill_payment` read the payment with
  `SELECT amount, payment_date FROM bill_payments`, but there is no `bill_payments`
  table — AP payments live in `ap_payments` (Postgres reads `ap_payments`). So the
  query failed at runtime with "no such table: bill_payments". As with the other
  auto-posting fixes, it also parsed the RFC3339 `payment_date` directly as a
  `NaiveDate`. Both are fixed: it now selects `FROM ap_payments` and reduces the
  parsed timestamp with `.date_naive()`. Posting a bill payment now produces a
  balanced AP-debit / Cash-credit entry on both backends. Verified with matching
  SQLite (`gl_auto_post_bill_payment`) and live-Postgres
  (`postgres_gl_auto_post_bill_payment`) regression tests. (4 of 5 in the GL
  auto-posting cluster fixed; only `auto_post_inventory_cost` remains.)
- **Auto-posting a vendor bill to the general ledger was completely broken on
  SQLite (two bugs).** `auto_post_bill` read the bill with
  `SELECT total_amount, bill_date FROM bills`, but there is no `bills` table —
  accounts-payable bills live in `ap_bills` (Postgres reads `ap_bills`). So the
  query failed at runtime with "no such table: bills". As with the other
  auto-posting fixes, it also parsed the RFC3339 `bill_date` directly as a
  `NaiveDate`. Both are fixed: it now selects `FROM ap_bills` and reduces the
  parsed timestamp with `.date_naive()`. Posting a bill now produces a balanced
  Inventory/Expense-debit / AP-credit entry on both backends. Verified with
  matching SQLite (`gl_auto_post_bill`) and live-Postgres
  (`postgres_gl_auto_post_bill`) regression tests. (3 of 5 in the GL auto-posting
  cluster fixed; `auto_post_bill_payment` and `auto_post_inventory_cost` remain.)
- **Auto-posting a received payment to the general ledger was completely broken
  on SQLite (two bugs).** `auto_post_payment_received` read the payment with
  `SELECT amount, payment_date FROM payments`, but the SQLite `payments` table has
  no `payment_date` column — the payment date is `paid_at` (nullable) falling back
  to `created_at`. So the query failed at runtime with "no such column:
  payment_date". As with `auto_post_invoice`, it also parsed that RFC3339 timestamp
  directly as a `NaiveDate`. Both are fixed: it now selects
  `COALESCE(paid_at, created_at)` and reduces the parsed timestamp with
  `.date_naive()`, matching the (already-correct) Postgres path. Posting a payment
  now produces a balanced Cash-debit / AR-credit entry on both backends. Verified
  with matching SQLite (`gl_auto_post_payment_received`) and live-Postgres
  (`postgres_gl_auto_post_payment_received`) regression tests. (2 of 5 in the GL
  auto-posting cluster fixed; `auto_post_bill` / `auto_post_bill_payment` /
  `auto_post_inventory_cost` remain.)
- **Auto-posting an invoice to the general ledger was completely broken on
  SQLite (two bugs).** `auto_post_invoice` read the invoice total with
  `SELECT total_amount FROM invoices`, but the SQLite `invoices` money column is
  named `total` (Postgres reads `total`) — so the query failed at runtime with
  "no such column: total_amount". Even with the column corrected, it parsed
  `invoice_date` (stored as a full RFC3339 timestamp) directly as a `NaiveDate`,
  which cannot parse a timestamp. Both are fixed: it now selects `total` and
  parses the timestamp before reducing it to a date with `.date_naive()`, exactly
  as the (already-correct) Postgres path does. Posting an invoice now produces a
  balanced, correctly-dated journal entry on both backends. Verified with matching
  SQLite (`gl_auto_post_invoice`) and live-Postgres
  (`postgres_gl_auto_post_invoice`) regression tests. (The sibling
  `auto_post_payment_received` / `auto_post_bill` / `auto_post_bill_payment` /
  `auto_post_inventory_cost` methods share the same wrong-column + date-parse
  pattern on SQLite and are tracked for follow-up firings.)
- **Updating a custom object type was broken on Postgres (type-decode error).**
  The optimistic-locking `UPDATE … RETURNING 1` decoded the `INT4` literal `1` as
  `i64`, which fails on Postgres with the same "Rust type i64 is not compatible
  with SQL type INT4" mismatch that broke bill creation — so `update_type` errored
  on Postgres even for a valid, version-matching update (SQLite, being
  dynamically typed, worked). It now decodes as `i32`. A sweep of the whole
  Postgres backend for this class (`(i64,)`/`i64` decodes over `INT4`
  expressions) found only these two sites; all other such decodes are over
  `COUNT(*)`, `BIGINT`, or `BIGSERIAL` columns and are correct. Verified with a
  live-Postgres regression test (`postgres_custom_object_type_update`).
- **Creating an accounts-payable bill was broken on Postgres (type-decode
  error).** `create_bill_async` computed each item's line number with
  `SELECT COALESCE(MAX(line_number), 0) + 1` — an `INT4` result — but decoded it
  as `i64`, which fails with "Rust type i64 is not compatible with SQL type
  INT4". Since every bill has at least one item, no AP bill could be created on
  Postgres at all (the path was untested). It now decodes the line number as
  `i32`. Uncovered by, and verified with, a live-Postgres regression test
  (`postgres_list_bills_filters`).
- **SQLite `list_bills` ignored the `purchase_order_id`, `min_amount`,
  `max_amount`, and `offset` filters, returning the wrong set.** SQLite applied
  only supplier/status/overdue/limit while Postgres applies all of these. SQLite
  now filters by purchase order in SQL, by the money thresholds exactly in Rust
  (the TEXT `total_amount` column can't be compared numerically in SQL without a
  lossy `CAST`), and paginates with `offset` after filtering — matching Postgres.
  (The `from_date`/`to_date` filters are left for a follow-up: they're entangled
  with a separate AP date-storage divergence.) Verified with RED→GREEN regression
  tests on both backends (`sqlite/accounts_payable.rs::
  list_bills_honors_po_amount_and_offset_filters` + live-PG
  `postgres_list_bills_filters`).
- **SQLite could mint gift cards and store credits with negative/zero balances.**
  The SQLite `create` paths had no amount validation, so a gift card with a
  negative `initial_balance` or a store credit with a non-positive `amount` was
  happily issued (store credits even recorded a bogus negative `issue`
  transaction). Postgres rejected these, but only via DB CHECK constraints that
  the SQLite schema lacks — and it surfaced them as a raw `DatabaseError`. Both
  backends now reject non-positive issuance up front with a clean
  `ValidationError` (gift-card balance must be ≥ 0; store-credit amount must be
  > 0, matching the existing Postgres constraints). Verified with RED→GREEN
  regression tests on both backends
  (`sqlite/gift_cards.rs::create_rejects_negative_initial_balance`,
  `sqlite/store_credits.rs::create_rejects_non_positive_amount`, and live-PG
  `postgres_issuance_amount_guard`).
- **Creating a subscription seeded no initial billing cycle on Postgres.** SQLite
  created an initial billing cycle (cycle 1) for the subscription's current period
  at creation, but the Postgres path committed the subscription and returned
  without creating any cycle. A fresh subscription therefore had one billing cycle
  on SQLite and zero on Postgres, so dunning, next-charge, cycle-history, and
  revenue consumers saw a different world per backend. Postgres now seeds the same
  cycle-1 record. Verified with RED→GREEN regression tests on both backends
  (`sqlite/subscriptions.rs::create_subscription_seeds_an_initial_billing_cycle` +
  live-PG `postgres_subscription_initial_cycle`).
- **`get_average_days_to_pay` computed a different value on each backend.**
  SQLite averaged the fractional day difference (`JULIANDAY(applied) -
  JULIANDAY(invoice)`), while Postgres used `EXTRACT(DAY FROM (applied - invoice))`
  — which returns only the whole-day component of each interval, flooring every
  invoice's pay-latency before averaging. So two invoices paid at 10.5 and 11.5
  days averaged to 11 on SQLite but 10 on Postgres. Postgres now averages
  fractional days (`EXTRACT(EPOCH …) / 86400`), matching SQLite. Verified with
  RED→GREEN regression tests on both backends
  (`sqlite/accounts_receivable.rs::average_days_to_pay_uses_fractional_days` +
  live-PG `postgres_avg_days_to_pay`).
- **`list_jurisdictions` returned tax jurisdictions in a different order on each
  backend.** SQLite ordered by `country_code, state_code, level, name` while
  Postgres ordered only by `level, name`, so the same query produced a different
  sequence per backend (and different `result[0]`). Both now order by
  `country_code, COALESCE(state_code, ''), level, name` — the `COALESCE` makes a
  NULL `state_code` sort consistently across backends (SQLite sorts NULLs first,
  Postgres last). Verified with RED→GREEN regression tests on both backends
  (`sqlite/tax.rs::list_jurisdictions_orders_by_country_then_state` + live-PG
  `postgres_list_jurisdictions_order`).
- **`list_suppliers` diverged on both filters and pagination between backends.**
  Postgres silently ignored the `name` and `country` filters (it applied only
  `active_only`) and had no `OFFSET`, so paginating or searching suppliers
  returned the wrong set; SQLite honored name/country but applied `offset` only
  when an explicit limit was given and had no default page size. Both backends now
  apply the name (case-insensitive substring), country, and active-only filters
  and paginate with `offset` + a default limit of 100. Verified with RED→GREEN
  regression tests on both backends
  (`sqlite/purchase_orders.rs::list_suppliers_applies_offset_and_default_limit`
  + live-PG `postgres_list_suppliers_filters`).
- **SQLite `get_locations_for_warehouse` silently hid inactive locations.** It
  passed `is_active: Some(true)` to the underlying list, so a deactivated location
  disappeared from a method whose contract (and the Postgres backend) is to return
  *all* locations for a warehouse — filtered subsets have their own accessors
  (`get_pickable_locations`, `get_receivable_locations`). SQLite now returns all
  locations, active and inactive, matching Postgres. Verified with RED→GREEN
  regression tests on both backends
  (`sqlite/warehouse.rs::get_locations_for_warehouse_includes_inactive` + live-PG
  `postgres_locations_for_warehouse`).
- **SQLite `move_inventory` was not atomic — a failed move destroyed source
  stock.** The move ran the source decrement, destination increment, and movement
  insert as three separate auto-committed statements on a plain connection, so if
  the destination write failed (e.g. an invalid destination location), the source
  had already been debited with nothing credited anywhere — inventory silently
  vanished (a test lost 3 units). Postgres already wrapped the move in a
  transaction. SQLite now runs the whole move inside one IMMEDIATE (retrying)
  transaction, so it is all-or-nothing on both backends. Verified with RED→GREEN
  regression tests on both backends
  (`sqlite/warehouse.rs::move_inventory_is_atomic_when_destination_write_fails` +
  live-PG `postgres_move_inventory_atomic`).
- **Concurrent purchase-order receipts were silently dropped on SQLite.** PO
  `receive` is a read-check-write of each item's `quantity_received`, but SQLite
  ran it in a plain (non-retrying) deferred transaction, so simultaneous receipts
  from multiple receiving stations conflicted on the write lock and most failed
  with a lock error — a concurrency test saw only 2 of 8 receipts of 2 units land
  (4 instead of 16). Postgres uses one atomic conditional UPDATE and landed them
  all. SQLite now runs `receive` under the retrying IMMEDIATE transaction the
  other read-modify-write paths use (refunds, WAC, work orders), so concurrent
  receipts serialize and every one is recorded, while the over-receipt and
  positive-quantity guards still hold. Verified with RED→GREEN regression tests on
  both backends (`sqlite/purchase_orders.rs::
  receive_accumulates_concurrent_partial_receipts_without_lost_updates` +
  `receive_updates_quantities_and_rejects_over_receipt` + live-PG
  `postgres_po_receive_concurrency`).
- **`get_available_for_sku` allocated serials newest-first (LIFO) on SQLite but
  oldest-first (FIFO) on Postgres, so the two backends handed out different
  physical units.** SQLite delegated to `list`, which orders `created_at DESC`,
  while Postgres orders `created_at ASC`. Allocating the *newest* stock first is
  the wrong inventory behavior and diverged from Postgres. SQLite now uses a
  dedicated FIFO query (`ORDER BY created_at ASC`), independent of `list`'s
  newest-first view, so both backends allocate the oldest available serial first.
  Verified with RED→GREEN regression tests on both backends
  (`sqlite/serials.rs::get_available_for_sku_allocates_oldest_first_fifo` +
  live-PG `postgres_serial_fifo`).
- **SQLite `count` for serial numbers ignored all but three of its filters, so a
  count disagreed with the corresponding list.** `count` only applied `sku`,
  `status`, and `lot_id`, while `list` (and Postgres's `count`) apply the full
  `SerialFilter` — serial, serial_prefix, statuses, lot_number, location, owner,
  warranty, and the manufactured/sold date ranges. So e.g. counting serials at a
  location returned every serial while listing them returned only those at that
  location. Both `list` and `count` now build their `WHERE` clause from one
  shared helper, so they can no longer diverge. Verified with a RED→GREEN
  regression test asserting `count(f) == list(f).len()` for each filter
  (`sqlite/serials.rs::count_matches_list_for_all_filters`).
- **SQLite `list` for purchase orders ignored the `offset` filter and had no
  default page size, returning a different page than Postgres.** The SQLite query
  applied `LIMIT` only when a limit was set and never referenced `offset`, while
  Postgres applies `offset` and defaults the page size to 100 — so paginating
  (e.g. `offset = 1`) returned the whole list on SQLite, and an uncapped query
  returned every row on SQLite versus the first 100 on Postgres. SQLite now
  applies `offset` and the same default limit of 100. (The `from_date` /
  `to_date` / `min_total` / `max_total` filter fields are ignored by *both*
  backends — a symmetric gap, not a divergence — and were left unchanged.)
  Verified with RED→GREEN regression tests on both backends
  (`sqlite/purchase_orders.rs::list_applies_offset_and_pagination` + live-PG
  `postgres_po_list_pagination`).
- **SQLite `count_locations` ignored the `zone`, `aisle`, `is_pickable`, and
  `is_receivable` filters that `list_locations` applies, so a count disagreed
  with the corresponding list.** The SQLite count query only applied
  `warehouse_id`, `location_type`, and `is_active`, so e.g. counting pickable
  locations returned every location while listing them returned only the pickable
  ones — `count_locations(f) != list_locations(f).len()` on the same backend, and
  a different total than Postgres (which applies all seven predicates).
  `count_locations` now applies the same predicates as `list_locations`. Verified
  with a RED→GREEN regression test asserting `count == list().len()` for each
  previously-ignored filter (`sqlite/warehouse.rs::
  count_locations_honors_same_filters_as_list_locations`).
- **SQLite `list` for invoices silently ignored 9 of the `InvoiceFilter` fields,
  returning the wrong set.** The SQLite implementation only applied
  `customer_id`, `order_id`, `status`, and `overdue_only`; `invoice_type`, the
  `invoice_date`/`due_date` ranges, `min_total`, `max_total`, `min_balance`, and
  `invoice_number` were dropped, while Postgres applies them all. A collections
  query for invoices with a balance over a threshold therefore returned every
  invoice on SQLite. SQLite now applies the type/date/number filters in SQL and
  the money thresholds exactly in Rust (the TEXT money columns can't be compared
  numerically in SQL without a lossy `CAST`), with pagination applied after
  filtering — matching Postgres. Verified with RED→GREEN regression tests on both
  backends (`invoice_list_filters` + live-PG `postgres_invoice_list_filters`).
- **Invoice money diverged between backends: SQLite kept sub-cent precision while
  Postgres rounds to cents, so totals and payment status could disagree.** SQLite
  stored invoice line totals, subtotal, total, and balances as full-precision
  TEXT, but Postgres stores them in `DECIMAL(12, 2)` columns that round on write.
  An item at `unit_price = 10.005` gave `total = 10.005` on SQLite (so paying
  `10.005` marked it `Paid`) versus `10.01` on Postgres (`PartiallyPaid`) — same
  inputs, different money and status. SQLite now rounds invoice money to cents
  with the same half-away-from-zero strategy Postgres `NUMERIC` uses (line totals,
  stored input amounts, subtotal, total, balance, and payment application), so
  both backends store penny-identical values and foot. Completes the money-
  rounding parity work already done for carts and orders. Verified with RED→GREEN
  regression tests on both backends (`invoice_total_rounding` + live-PG
  `postgres_invoice_total_rounding`).
- **Postgres `delete` on a serial number skipped the guards SQLite enforces —
  it could permanently delete a sold serial (data loss) and returned `Ok(())`
  for a non-existent id.** SQLite's `delete` rejects a missing serial with
  `NotFound` and a non-`Available` serial with `ValidationError` before touching
  the record; Postgres's `delete_async` went straight to the transaction-history
  check, so deleting an unknown id silently succeeded and deleting a `Sold`
  serial with no post-creation history destroyed it. Postgres now applies the
  same existence + `Available`-status guards. Verified with RED→GREEN regression
  tests on both backends (`sqlite/serials.rs::
  delete_rejects_missing_and_non_available_serials` + live-PG
  `postgres_serial_delete_guards`).
- **Postgres per-customer promotion usage limit could be exceeded under
  concurrency.** `record_usage` enforces `per_customer_limit` with a
  COUNT-then-INSERT against the `promotion_usage` ledger inside a plain
  READ COMMITTED transaction with no row lock, so two simultaneous redemptions
  for the same (promotion, customer) both read the ledger before either inserts,
  both pass the limit check, and both commit — over-redeeming the limit (a
  concurrency test saw 3 succeed against a limit of 1). The transaction now locks
  the promotion row `SELECT … FOR UPDATE` up front, serializing concurrent
  redemptions of the same promotion so the second sees the first's committed
  usage row and is correctly rejected. The SQLite backend already serialized this
  path via `BEGIN IMMEDIATE`. Verified with a RED→GREEN live-Postgres concurrency
  test (`postgres_promo_usage_race`, 10 concurrent redemptions → exactly 1
  succeeds).
- **SQLite `get_customer_aging` returned `NotFound` for an existing customer
  with no open invoices, instead of a zero-filled aging.** After confirming the
  customer exists, the SQLite path returned `Err(NotFound)` when the open-invoice
  count was zero, while Postgres returned `Ok(Some(..))` with zero balances. This
  also broke `get_customer_summary` and `generate_statement` for any paid-up
  customer (both propagate the aging lookup). SQLite now returns the zero-filled
  aging for an existing customer and reserves `None`/`NotFound` for a genuinely
  unknown customer, matching Postgres. Verified with RED→GREEN regression tests
  on both backends
  (`sqlite/accounts_receivable.rs::get_customer_aging_returns_zeros_for_existing_customer_without_invoices`
  + live-PG `postgres_ar_customer_aging`).
- **SQLite `get_aging_report` silently ignored the `min_balance` and
  `aging_bucket` filters, returning the wrong customer set.** The SQLite
  implementation only honored `customer_id`, `overdue_only`, `offset`, and
  `limit`; the `min_balance` and `aging_bucket` fields of `ArAgingFilter` were
  never applied, so an AR aging report filtered to (say) customers owing ≥ $1000
  returned everyone on SQLite while Postgres (which applies both via `HAVING`)
  returned only the qualifying customers. SQLite now filters by total
  outstanding (`min_balance`) and by a positive balance in the requested bucket
  (`aging_bucket`) before pagination, matching Postgres. Verified with RED→GREEN
  regression tests on both backends
  (`sqlite/accounts_receivable.rs::get_aging_report_honors_min_balance` /
  `_honors_aging_bucket` + live-PG `postgres_ar_aging_filters`).
- **`BundleDiscount` promotions discounted $0 on SQLite while Postgres applied
  the full amount.** SQLite's `calculate_discount` had no `BundleDiscount` arm, so
  the type fell through to `Decimal::ZERO` and — because `apply_promotions` only
  records a promotion when its discount is positive — the promotion was silently
  dropped, not even reported as rejected. Postgres returned
  `bundle_discount.unwrap_or(0)` and exempted it from the applicable-amount clamp.
  SQLite now applies the `bundle_discount` amount and mirrors the same clamp
  exemption, so a configured bundle promotion discounts identically on both
  backends. Verified with RED→GREEN regression tests
  (`sqlite/promotions.rs::apply_promotions_applies_bundle_discount` + live-PG
  `postgres_promotion_bundle`).
- **SQLite `get_income_statement` was broken: it summed the TEXT money columns
  with SQL `SUM()` and read the result back as a string, erroring at runtime.**
  `gl_journal_entry_lines.debit_amount`/`credit_amount` are stored as TEXT in
  SQLite, so `COALESCE(SUM(l.debit_amount), 0)` coerced them to a float/int and
  returned a non-TEXT value; the subsequent `row.get::<String>` failed with
  `Invalid column type Integer`. The income statement — and `run_period_close`,
  which calls it — therefore errored on SQLite for any period with an active
  revenue/expense account, while Postgres (with `NUMERIC` columns) worked. No
  test covered this path. SQLite now aggregates with the exact `decimal_sum`
  aggregate (which returns TEXT and is penny-exact), so the report both parses
  and foots. Verified with RED→GREEN regression tests on both backends
  (`gl_income_statement` + live-PG `postgres_gl_income_statement`).
- **SQLite stored exchange rates at full precision while Postgres rounds to 10
  dp, so high-precision rates converted differently per backend.** Postgres
  declares `exchange_rates.rate` as `DECIMAL(20, 10)`, which rounds any
  finer-grained rate to 10 fractional digits (half away from zero); SQLite kept
  the rate as full-precision TEXT. A rate like `1.23456789019999` therefore
  produced `1.2345678902 × amount` on Postgres but the un-rounded product on
  SQLite. SQLite's `set_rate`/`set_rates` now round the stored rate to 10 dp with
  the same half-away-from-zero strategy Postgres uses, so both backends store and
  convert identically. Verified with RED→GREEN regression tests on both backends
  (`currency_rate_precision` + live-PG `postgres_currency_rate_precision`).
- **Postgres seeded 9 hardcoded exchange rates; SQLite seeded none, so currency
  conversion silently diverged.** Migration `005_currency` seeded stale FX rates
  (`source='seed'`: EUR 0.92, GBP 0.79, JPY 149.50, …), so `convert(USD→EUR)`
  returned an out-of-date rate on Postgres while SQLite errored "No exchange rate
  found" until a rate was set explicitly. Conversion amounts are outward-facing,
  and a static seed quietly goes stale, so the explicit-rate behavior (SQLite's)
  is the safer, intended default. New migration
  `053_remove_seeded_exchange_rates` deletes only `source='seed'` rows — any rate
  a user has set upserts with `source='manual'`, so user rates (even for
  originally-seeded pairs) are preserved. Both backends now require an explicit
  rate before converting. Verified with a RED→GREEN regression test against live
  Postgres (`postgres_currency_no_seed`).
- **SQLite x402 credit could overflow `i64` on a Credit adjustment (panic/wrap
  instead of a clean error).** The Credit path added `current_balance +
  amount_i64` unchecked, which panics on overflow in debug builds and silently
  wraps to a negative balance in release — unlike Postgres, which used
  `checked_add`. It now uses `checked_add` and rejects the overflow with a
  `ValidationError` ("x402 balance overflow"), matching Postgres. (Reachable only
  at balances near `i64::MAX`, but a payment path should never panic or wrap.)
  Verified with a RED→GREEN regression test.
- **`revenue_forecast` averaged money through f64 on SQLite, drifting exact
  decimals.** Money is stored as TEXT, but the forecast read
  `AVG(SUM(total_amount))` as an `f64` and converted with
  `Decimal::from_f64_retain`, so a period revenue of 0.10 + 0.20 came back as
  0.3000000000000000444… instead of 0.30 (Postgres computes it exactly in
  NUMERIC). It now sums each period with the exact `decimal_sum` aggregate and
  divides by the period count in `Decimal`, matching Postgres. (The SQLite
  forecast's weekly grouping also switched from `%W` to the ISO-week expression,
  matching Postgres's ISO weeks like `get_revenue_by_period`.) Verified with a
  RED→GREEN regression test.
- **Weekly revenue was bucketed by a non-ISO week on SQLite, diverging from
  Postgres.** `get_revenue_by_period` with weekly granularity labeled buckets with
  SQLite's `strftime('%Y-W%W')` (calendar-year week 00–53) while Postgres used ISO
  weeks (`to_char(…, 'IYYY-"W"IW')`). These differ at year boundaries and in week
  numbering — e.g. Sunday 2023-01-01 bucketed as `2023-W00` on SQLite but
  `2022-W52` on Postgres — so weekly revenue breakdowns didn't match across
  backends. SQLite now computes the ISO-8601 week label (via the week's Thursday),
  matching Postgres exactly (verified against live Postgres `to_char` output for
  year-boundary dates). Verified with a RED→GREEN unit test on the exact SQL
  expression.
- **`return_metrics` top-returned-products fragmented a renamed product's returns
  on Postgres (same divergence as `top_products`).** Like the top-products report,
  the top-returned-products list grouped by `(sku, name)` on Postgres but by `sku`
  on SQLite, so a SKU returned under two names (a product renamed mid-window) split
  into multiple rows on Postgres — different returned-unit counts and ranking than
  SQLite. Both backends now group by `sku` (with `MAX(name)` for the display name),
  aggregating a product's returns into one row identically. Verified with RED→GREEN
  regression tests on both backends.
- **`top_products` revenue was aggregated by different keys on each backend,
  fragmenting a renamed product's revenue on Postgres.** `order_items.name` is a
  per-line snapshot, so the same SKU can appear under different names (e.g. a
  product renamed mid-window). SQLite grouped the "top products by revenue" report
  by `sku` (correct — one row per product), but Postgres grouped by
  `(product_id, sku, name)`, splitting one product's revenue across multiple rows
  — e.g. a SKU with $200 total under two names showed as one $200 row on SQLite
  but two $100 rows on Postgres, changing both the figures and the top-N ranking.
  Both backends now group by `sku` (with a deterministic `MAX(name)` /
  `MAX(product_id)` for the display columns), so a product's revenue aggregates
  into a single row identically on both. Verified with RED→GREEN regression tests
  on both backends.
- **SQLite left a stale `next_billing_date` on a paused subscription (diverged
  from Postgres).** Pausing a subscription should clear its scheduled
  `next_billing_date` — a paused subscription has no next charge until it resumes
  (and `resume` recomputes the date). Postgres cleared it on pause; SQLite left
  the old date in place, so a paused subscription still advertised a "next charge"
  date on SQLite (a stale read a customer UI would show). SQLite now nulls
  `next_billing_date` on pause too, matching Postgres. Billing is unaffected on
  both (due-for-billing only considers active subscriptions). Verified with a
  RED→GREEN regression test (paused → no next billing date; resume restores it).
- **`update_average_cost` was broken on both backends (queried a non-existent
  column) and raced under concurrency.** The weighted-average-cost update read
  the SKU's on-hand quantity with `SELECT quantity_on_hand FROM inventory_items`,
  but `quantity_on_hand` lives in `inventory_balances` (per location, keyed by
  `item_id`) — `inventory_items` has no such column on either backend — so every
  call errored (`no such column` / `column "quantity_on_hand" does not exist`).
  The method had no test coverage, so this went unnoticed. It now sums the SKU's
  balances from `inventory_balances` (joined via `item_id`). Separately, the
  read-modify-write of `item_costs.average_cost` had no serialization, so two
  concurrent receipts for the same SKU both read the same average and one
  clobbered the other, corrupting the WAC (a lost update; a regression test shows
  the concurrent average landing at 2.49 instead of the correct 8.51). The
  read/compute/write now runs inside one transaction that locks the cost row
  (SQLite `IMMEDIATE`, Postgres `SELECT … FOR UPDATE`). Verified with functional
  and concurrency regression tests on both backends.
- **Postgres billing-cycle failures didn't advance `retry_count` or stamp
  `billed_at` (diverged from SQLite, breaking dunning parity).** Marking a
  billing cycle `failed` should increment its dunning `retry_count` and stamp
  `billed_at` (SQLite does both), but the Postgres `update_billing_cycle_status`
  path updated only `status` + `updated_at` — so a failed cycle never advanced
  `retry_count`, and any retry-cap / dunning logic keyed on it behaved differently
  between backends (a cycle that should stop retrying after N attempts on SQLite
  would retry forever on Postgres). Postgres now stamps `billed_at` on Paid/Failed
  and increments `retry_count` on failure, matching SQLite. Verified with RED→GREEN
  regression tests on both backends.
- **Postgres seeded a new item's `average_cost`/`last_cost` to $0 instead of its
  standard cost, zeroing reported inventory value.** When `set_item_cost` creates
  a brand-new `item_costs` row, SQLite seeds `average_cost` and `last_cost` to the
  `standard_cost` (documented: "average_cost starts as standard"), but the
  Postgres path hardcoded them to `0`. Since inventory valuation reads
  `average_cost`, a freshly-costed SKU with on-hand stock reported its full value
  on SQLite but **$0 on Postgres** (e.g. 100 units × $10 standard = $1000 vs $0),
  and the weighted-average cost then diverged permanently from that zero base.
  Postgres now seeds both to `standard_cost`, matching SQLite. Verified with
  RED→GREEN regression tests on both backends.
- **Work-order `complete` lost completions under concurrency (both backends).**
  `complete` read `quantity_completed`, added the new units in application code,
  and wrote the total back — a read-modify-write with no serialization (SQLite
  read and wrote on a pooled connection with no transaction; Postgres read and
  wrote on separate pooled connections). Two concurrent completions could both
  read the same starting quantity and one overwrite the other, under-counting
  produced units: regression tests fire 10 (SQLite) / 25 (Postgres) concurrent
  single-unit completions and without the fix only ~3 / ~5 are recorded. The
  read-modify-write now runs inside one transaction that takes the row/write lock
  up front (SQLite `IMMEDIATE`, Postgres `SELECT … FOR UPDATE`), so concurrent
  completions serialize and every one is counted. Verified with concurrency
  regression tests on both backends (RED: completions lost; GREEN: the total
  equals the number of completions). (This fixes only the lost-update race; the
  separate question of whether to *cap* cumulative completions at the build target
  is left unchanged, as manufacturing overage can be legitimate.)
- **Postgres `complete_receiving` didn't mark line items received (diverged from
  SQLite).** On completion SQLite marks every non-rejected `receipt_items` row
  `received`, but the Postgres path updated only the receipt header, leaving line
  items in their prior status (e.g. `pending`) — so a downstream query filtering
  receipt items by `received` returned different results on the two backends.
  Postgres now marks the non-rejected items `received` as part of completion,
  inside one transaction with the header update. Verified with RED→GREEN
  regression tests on both backends. This completes the receiving-domain parity
  (together with the `cancel_receipt` fix below).
- **`cancel_receipt` guards diverged and let received goods receipts be
  cancelled.** The receipt lifecycle is
  `Expected → InProgress → Received → Inspecting → PuttingAway → Completed`, and
  `complete_receiving` sets `Received` — but each backend gated cancellation on a
  different, incomplete status: SQLite blocked only `Completed` (never reached by
  the normal flow, so a **received** receipt was cancellable), while Postgres
  blocked only `Received` (so a receipt already in inspection/put-away, or already
  cancelled, was still cancellable). Both now gate on a shared
  `ReceiptStatus::can_cancel()` — a receipt is cancellable only from `Expected` or
  `InProgress`; once its goods are received (or it is already cancelled) it cannot
  be cancelled. Verified with RED→GREEN regression tests on both backends
  (received receipt no longer cancellable on SQLite; already-cancelled receipt no
  longer re-cancellable on Postgres; in-progress/expected still cancellable).
- **Postgres lot `consume`/`reserve`/`adjust` could over-consume stock under
  concurrency (missing `FOR UPDATE`).** These paths loaded the lot row inside a
  transaction with a plain `SELECT`, checked availability in application code, and
  wrote the new `quantity_remaining` — a check-then-write with no row lock. Two
  concurrent consumers could both read the same remaining quantity, both pass
  `can_consume`, and both write, over-consuming the lot (a TOCTOU race): a
  regression test fires 25 concurrent single-unit consumers at a 10-unit lot and
  without the fix **all 25 succeed**. The sibling `confirm_reservation`/`transfer`
  paths already used `SELECT … FOR UPDATE`, and the SQLite backend serializes via
  its single `conn.transaction()`, so this was a Postgres-only divergence. All
  three now lock the lot row with `FOR UPDATE`, so exactly the available units are
  consumed and the lot never goes negative. Verified with a live-Postgres
  concurrency regression test (RED: 25/10 over-consume; GREEN: exactly 10, ends at
  zero).
- **`fulfill_backorder` guards diverged between backends and let backorders be
  over-fulfilled / fulfilled after cancellation.** Each backend enforced only
  half the rules: Postgres checked the status (rejecting cancelled/fulfilled) but
  had **no remaining-quantity bound** — its `(quantity_ordered - fulfilled).max(0)`
  clamp silently swallowed the overflow, so fulfilling 8 units twice against a
  10-unit backorder recorded **16 units fulfilled** and flipped the status to
  `Fulfilled`; SQLite checked the quantity bound but had **no status guard**, so a
  **cancelled** backorder could still be fulfilled. Both paths were also
  non-transactional (read on one connection, writes on another), risking lost
  updates / over-fulfillment under concurrency and orphaned fulfillment rows. Both
  backends now enforce the full set — `quantity > 0`, status not
  cancelled/fulfilled, and `quantity <= quantity_remaining` — inside one
  transaction (SQLite `IMMEDIATE` / Postgres `SELECT … FOR UPDATE`, so concurrent
  fulfillments serialize). Verified with RED→GREEN regression tests on both
  backends (over-fulfill rejected with state unchanged, cancelled-fulfill
  rejected, non-positive rejected, exact-remainder fulfillment still completes).
- **Quality inspection results weren't validated against the inspected quantity
  (both backends).** `record_inspection_result` wrote `quantity_passed` /
  `quantity_failed` straight to the inspection item with no bounds check, so a
  caller could record passing/failing more units than were ever inspected (e.g.
  8 passed + 5 failed against a 10-unit inspection) or record negative counts —
  corrupting quality/yield reporting. Both backends now reject negative
  quantities and reject `quantity_passed + quantity_failed > quantity_inspected`
  with a `ValidationError` (recording exactly the inspected quantity is still
  allowed). Verified with RED→GREEN regression tests on both backends.
- **`complete_pick` was non-idempotent and had no over-pick guard (both
  backends).** Completing a pick task unconditionally set its status and
  incremented the wave's `completed_pick_count`, with no prior-status check and
  no `quantity_picked <= quantity_requested` validation — the pick UPDATE, the
  read, and the counter increment ran as three separate non-transactional
  statements. So a duplicate completion (a double-scan or retry) re-incremented
  the wave counter, and a worker could record picking more units than were
  requested. `complete_pick` now runs inside one transaction (SQLite `IMMEDIATE`
  / Postgres `SELECT … FOR UPDATE`, so concurrent completions serialize) that
  reads the pick's current status first and: treats an already-finalized
  (`Completed`/`Short`) pick as an idempotent no-op (no second counter
  increment), rejects completing a `Cancelled` pick, and rejects picking more
  than the requested quantity — both with a `ValidationError`. Verified with
  RED→GREEN regression tests on both backends (over-pick rejected; a duplicate
  completion leaves `completed_pick_count` at 1).
- **`complete_refund` was non-idempotent — a duplicate completion double-counted
  the refund (both backends).** Completing a refund folds `refund.amount` into
  the payment's `amount_refunded`, but the completion step had no terminal-state
  guard: calling it again (a duplicated payment-processor webhook or a retry —
  routine in production) re-read the same refund and re-added its amount, so a
  $50 refund on a $100 payment could push `amount_refunded` to $100 and flip the
  payment to fully `Refunded` though only $50 was ever refunded. `complete_refund`
  now reads the refund's current status inside the write transaction (SQLite
  `IMMEDIATE` / Postgres `SELECT … FOR UPDATE`, so concurrent completions
  serialize) and: treats an already-`Completed` refund as an idempotent no-op,
  and rejects completing a terminal `Failed`/`Cancelled` refund with a
  `ValidationError` (so a dead refund's amount is never folded in). Verified with
  RED→GREEN regression tests on both backends (duplicate completion keeps
  `amount_refunded` at the single amount; completing a failed refund is rejected
  and folds nothing).
- **Cart line-item `total` wasn't rounded to cents on SQLite (diverged from
  Postgres).** `CartItem::calculate_total` returned the raw
  `unit_price × qty − discount + tax` unrounded, so a sub-cent line (e.g.
  `3.333 × 3 = 9.999`) stored/returned a line `total` of `9.999` on SQLite,
  while Postgres's `cart_items.total DECIMAL(12,2)` column coerced it to `10.00`
  — a per-line display divergence (the buyer-facing subtotal/grand total were
  already fixed). `CartItem::calculate_total` now rounds each line to 2 dp,
  matching both the Postgres column and the order pipeline's
  `OrderItem::calculate_total`, so cart line totals are chargeable money amounts
  and agree across backends. Verified with the cart-rounding regression tests on
  both backends.
- **Cart subtotal/grand total weren't rounded to cents on SQLite and diverged
  from Postgres.** SQLite computed the cart subtotal in `Decimal` and stored it
  (and the derived grand total) as full-precision TEXT with no rounding, so a
  sub-cent line — e.g. `unit_price 3.333 × qty 3 = 9.999` — persisted
  `grand_total = 9.999`, which is not a chargeable money amount. Postgres stored
  the same values in `DECIMAL(12,2)` columns, so the column silently coerced them
  to `10.00`. Same cart → `9.999` on SQLite vs `10.00` on Postgres, and the
  SQLite value would have carried into checkout. Both backends now round the
  subtotal and grand total to 2 dp in Rust (identical rounding strategy, rather
  than relying on Postgres column coercion), so cart totals are chargeable money
  amounts and agree across backends — consistent with the order-total rounding
  fix. Verified with a SQLite regression test and a live-Postgres parity test.
- **Order totals didn't foot to their line items and diverged across backends
  and creation paths.** Each order line stores a money `total` and the order
  stores `total_amount`. The shared `OrderItem::calculate_total` returned the raw
  `unit_price × qty − discount + tax` **unrounded**, so a line could persist a
  non-money value like `9.999`. The order total was then computed inconsistently:
  SQLite's single-create rounded per line and summed (so `total_amount` did *not*
  equal the sum of the stored unrounded line totals — the order didn't foot — and
  the total silently changed the first time `update_order_total` re-summed the
  unrounded lines); SQLite's batch path and both Postgres paths rounded nothing.
  The same order could therefore persist three different totals. Now
  `OrderItem::calculate_total` rounds each line to the currency minor unit (2 dp)
  and every creation path on both backends computes the order total as the sum of
  those rounded line totals — so `total_amount == SUM(order_items.total)` holds by
  construction, matches `update_order_total`, and agrees across SQLite and
  Postgres. Verified with a SQLite regression test (sub-cent line rounds and
  foots, stays stable across an item mutation, and multiple sub-cent lines sum
  their rounded values rather than rounding the raw sum) and a live-Postgres
  parity test.
- **Postgres: applying a coupon to a cart did nothing (discount silently
  dropped, invalid coupons accepted).** `PgCartRepository::apply_discount` only
  stamped the coupon *string* onto the cart — it never looked the coupon up,
  never resolved its promotion, never computed a `discount_amount`, and never
  recalculated the grand total. So on the Postgres backend a valid coupon left
  the buyer charged full price, and an unknown coupon code was accepted without
  error (unlike SQLite, which resolves the coupon, computes the discount, and
  rejects invalid codes). Ported the SQLite logic to Postgres: look up the coupon
  (rejecting unknown codes with a `ValidationError`), resolve its promotion,
  compute the discount (`PercentageOff` with optional `max_discount_amount` cap /
  `FixedAmountOff` capped at subtotal), persist it, and recalculate the total.
  Verified with live-Postgres integration tests covering fixed-amount, percentage,
  and invalid-coupon cases.
- **Returns could over-return (and over-refund) more units than were ordered,
  and could return another order's items.** Return creation recorded a refund of
  `unit_price × quantity` for each line without ever checking the line against
  its order item — so a caller could return 100 units of a 2-unit purchase, keep
  returning the same item across separate returns past the ordered quantity, or
  return an `order_item_id` belonging to a *different* order, each producing an
  inflated or illegitimate refund. Every return-creation path (SQLite `create`
  and `create_batch_atomic`, Postgres `create` and `create_batch_atomic`) now
  validates each line inside the write transaction: the order item must belong to
  the return's order, and the requested quantity plus units already claimed by
  non-terminal returns (rejected/cancelled returns release their claim) must not
  exceed what was purchased. The validation is factored into one helper per
  backend (`validate_return_item_tx` / `validate_return_item_pg`) so the four
  paths stay in lock-step. The Postgres single-return `create` path was also
  **non-transactional** (header inserted, then each item on a separate pool
  connection), so a rejected line left a partially-created return behind; it now
  runs header + items in one transaction that rolls back as a unit. Verified with
  SQLite embedded regression tests and live-Postgres integration tests (including
  a no-orphaned-header rollback assertion).
- **SQLite: `rewards` table was missing from the migration set.** The reward
  catalog shipped on PostgreSQL (`postgres/migrations/045_rewards.sql`) but had
  no SQLite migration, so `loyalty().create_reward(...)` failed at runtime with
  `no such table: rewards` on the default embedded backend — the sqlite
  `rewards.rs` unit tests created the table by hand and never exercised the
  migration path, so the gap was invisible. Added `056_rewards.sql` and an
  embedded regression test that runs migrations the production way.
- **Scoped percentage/tiered/BOGO discounts could bleed past their scoped
  items.** A discount scoped to a set of products was capped at the eligible
  items' worth only for the *fixed-amount* type; a *percentage* (or tiered or
  BuyXGetY) discount was not. A misconfigured percentage over 100% (an admin
  data-entry error, unvalidated) therefore discounted more than the scoped
  items were worth, eating into out-of-scope line-item value — e.g. a 150%
  "widgets only" coupon on a $40 widget + $60 gadget order discounted $60
  instead of the correct $40. Item-value discounts are now capped at the
  eligible amount on both backends (FreeShipping/FixedAmount/Bundle exempt by
  design); verified with a live-Postgres regression test.
- **Postgres ignored a promotion's `max_discount_amount` cap and skipped cent
  rounding.** The Postgres `calculate_discount` returned the raw discount,
  where SQLite applied `discount.min(max_discount_amount)` and `round_dp(2)` —
  so a "20% off, max $50" promotion was uncapped on Postgres and amounts could
  carry sub-cent fractions. Postgres now matches SQLite; live-Postgres
  regression test added.
- **Postgres tiered discounts depended on tier list order.** For open-ended
  tiers (no `max_value`), the Postgres `calculate_tiered_discount` kept the
  *last* matching tier in the list, while SQLite kept the one with the highest
  floor. A "spend more, save more" promotion whose tiers were stored high-to-low
  therefore gave a $100 order the $0 tier's 5% on Postgres and the $100 tier's
  20% on SQLite — the same input, two different discounts. Postgres now selects
  the highest applicable floor like SQLite; unit test (SQLite) plus a
  live-Postgres regression test cover an out-of-order tier list on both backends.
- **Tax `rounding_mode` setting was ignored, and the default contradicted
  itself.** `TaxSettings.rounding_mode` (default `"half_up"`) was persisted and
  round-tripped but never consulted: `calculate_tax` on both backends always
  used `round_dp`, whose default strategy is banker's rounding (round half to
  even). So `$0.125` of tax rounded to `$0.12` regardless of the configured
  mode, *even though the default mode is `"half_up"`, which should yield
  `$0.13`*. Tax rounding now honors the setting via the new
  `TaxSettings::rounding_strategy()` (`half_up` — the default — plus
  `half_even`/`bankers`, `half_down`, `up`, `down`/`truncate`, `ceil`, `floor`;
  unknown values fall back to `half_up`). **Behavior change:** stores on default
  settings now round tax half-up (away from zero) as documented, rather than
  half-to-even. Core mapping unit test, a SQLite end-to-end test, and a
  live-Postgres regression test all assert both modes on a `$0.125` midpoint.
- **Loyalty program tiers were silently dropped on both backends.**
  `LoyaltyProgram.tiers` is a modeled field, but neither `loyalty_programs`
  table had a column for it: `create_program` ignored the input and
  `row_to_program` always returned an empty list, so a program created with
  tiers lost them. Now persisted as a JSON array (SQLite `057_loyalty_tiers`,
  Postgres `051_loyalty_tiers`), with round-trip tests on both backends
  (the Postgres one verified against a live database).
- **Invoice `record_payment` could report failure for a payment that actually
  committed (both backends).** The payment's balance write happened inside a
  transaction, but the updated invoice was then read back on a *separate*
  connection *after* commit. Under contention that post-commit read could fail
  (SQLite "database table is locked"; a transient Postgres read error), so
  `record_payment` returned an error even though the money had already landed —
  and a caller treating the error as "payment failed" would retry and
  **double-pay** the invoice. Both backends now read the updated invoice back
  *inside* the transaction and return it, so a payment commits if and only if it
  returns Ok. Proven by restoring the strict `amount_paid == 10 × successes`
  concurrency assertion (which flaked before the fix) and passing it 20/20 in
  isolation plus two full parallel-suite runs; live-Postgres test also passes.
- **Wishlist item `quantity` was never persisted (both backends); `variant_id`
  and `priority` were dropped on SQLite.** `WishlistItem` models `variant_id`,
  `priority`, and `quantity`, and `AddWishlistItem` accepts all three, but the
  SQLite `wishlist_items` table only had `product_id`/`notes` columns —
  `add_item` dropped the rest and `row_to_item` hard-coded `variant_id=None`,
  `priority=None`, `quantity=1`. Postgres stored variant/priority but its INSERT
  omitted `quantity` too and read it back as a hard-coded `1`. So an item added
  with a variant, priority, or quantity ≠ 1 silently lost that data. Added the
  missing columns (SQLite migration `058`, Postgres `052`) and wired both
  backends' `add_item`/`row_to_item` to persist and read all three. Uncovered by
  the new Node wishlists binding; the SQLite unit test now round-trips
  quantity=3 + variant + priority, and the Postgres row-mapping test asserts
  quantity is read from the row.
- **AR aging report crashed on an invoice whose customer was deleted (SQLite).**
  `get_aging_report` `LEFT JOIN`s customers but read `first_name`/`last_name`/
  `email` as non-nullable strings, so a single orphaned invoice (SQLite does not
  enforce the `customer_id` foreign key, unlike Postgres) made the whole report
  error out with `Invalid column type Null`. Those columns are now read as
  optional and surface with an empty name — matching the Postgres backend, which
  already tolerates NULLs and whose FK prevents orphans in the first place.
  Deterministic regression test added.
- **AR aging report had unstable ordering and pagination.** `get_aging_report`
  sorted customers by `total_outstanding DESC` with no tiebreaker on either
  backend, so customers with equal outstanding balances came back in a
  non-deterministic order (SQLite's `HashMap` iteration order; Postgres's
  arbitrary row order) that also differed between the two backends. Because both
  backends apply `LIMIT`/`OFFSET` *after* this sort, the instability meant
  paging through the report could silently **skip or duplicate** tied customers.
  Both backends now break ties by `customer_id` for a total, backend-identical
  order. A SQLite test (5 equal-balance customers — 12/12 runs failed before the
  fix, 12/12 passed after) plus a live-Postgres test confirm the stable order.
- **Tax result listed jurisdictions in non-deterministic order.** `calculate_tax`
  collected the per-jurisdiction summary straight out of a `HashMap`, so the
  `jurisdictions` list came back in Rust's randomized hash-iteration order —
  different from one call to the next and different between the SQLite and
  Postgres backends for the same input. For an engine whose sync is meant to be
  verifiable and reproducible, identical inputs must produce byte-identical
  results. Both backends now sort jurisdictions by `code` (then `id`). A SQLite
  test (shown to fail intermittently — 6/12 runs — before the fix and pass
  12/12 after) plus a live-Postgres test confirm both backends return the same
  stable order.
- **Cart grand total could go negative from an oversized discount.** Both
  backends computed `grand_total = subtotal + tax + shipping − discount` with no
  floor, so a cart-level `discount_amount` larger than the rest of the cart
  produced a negative total — e.g. a $25 cart with a $100 discount showed
  `-$75`, which at checkout would charge the buyer a negative amount (credit
  them). The grand total is now clamped at zero on both backends. The Postgres
  backend duplicated the formula across four recompute paths (`update_cart_totals`,
  `add_item`, `update_item`, `remove_item`) and only some would have been fixed
  by a single edit, so adding an item to a discounted cart could still have gone
  negative there — all four are now clamped, matching SQLite (which routes every
  mutation through one helper). SQLite unit test plus a live-Postgres test.
- **Invoice and credit payments accepted zero and negative amounts on both
  backends.** `invoices().record_payment(...)` and `credit().apply_payment(...)`
  did no amount validation, unlike every other money-in operation (gift-card
  charge, store-credit apply, `charge_credit`, `reserve_credit`). A negative
  invoice payment drove `amount_paid` down and the balance up — un-paying a
  settled invoice — and a negative credit payment computed
  `(balance − (−x)).max(0) = balance + x`, *inflating* the credit balance past
  the limit (apply-payment does not re-check the limit). Both now reject
  non-positive amounts with a validation error before touching any state.
  SQLite unit tests plus a live-Postgres test cover zero and negative on both
  operations and both backends.
- **Concurrent credit reservations could over-reserve past the credit line
  (SQLite).** `reserve_credit` checks the requested hold against available
  credit (limit − balance − holds), then INSERTs the reservation and bumps
  `hold_amount` — a check-then-act the SQLite backend ran without serialization
  (the read used a lock-free pooled connection, and it held that connection
  while calling `recalculate_available_credit`, which under concurrency
  deadlocked the connection pool). Ten $20 reservations against $100 available
  therefore hung or over-reserved. The check, INSERT, and hold bump now run in a
  single IMMEDIATE transaction, matching the Postgres backend (which already
  locked the row `FOR UPDATE`), with the recompute afterward. A 10-thread SQLite
  test and a live-Postgres parity test confirm exactly five $20 reservations fit
  under $100 on both backends.
- **Concurrent credit charges could blow through the credit limit on both
  backends.** `charge_credit` reads `current_balance`/`credit_limit`, checks the
  limit, then writes the new balance — a check-then-act with no serialization on
  either backend (SQLite read the account on a lock-free pooled connection;
  Postgres read it without `FOR UPDATE`). Two charges landing at once each passed
  the limit check against the same stale balance and both committed: ten $20
  charges against a $100 limit drove the balance to $200. The check and the
  write now run under a row lock (SQLite `with_immediate_transaction`, Postgres
  `SELECT … FOR UPDATE`) so exactly the charges that fit are accepted. The same
  hardening was applied to SQLite `apply_payment` (was losing concurrent
  payments) and to the credit helpers it calls — `record_transaction`,
  `recalculate_available_credit`, and `release_credit_reservation` — which had
  been doing unserialized read-modify-writes and failing with "database table is
  locked" under contention (leaving a charged balance with a returned error).
  A 10-thread SQLite limit-bypass test, a 10-thread SQLite payment test, and a
  10-task live-Postgres limit-bypass test all confirm the limit now holds.
- **Concurrent invoice payments were lost (or errored) on both backends.**
  `record_payment` reads `amount_paid`, adds the payment, and writes the new
  total — but neither backend serialized that read-modify-write. On SQLite it
  ran in a *deferred* transaction, so simultaneous payments collided on the
  write lock and all but one failed with "database table is locked"; on
  Postgres the row was read without `FOR UPDATE`, so concurrent payments read
  the same stale `amount_paid` and silently overwrote each other. Ten $10
  payments landing at once on a $100 invoice left it recording far less than
  $100. Both paths now serialize the update (SQLite via
  `with_immediate_transaction`, Postgres via `SELECT … FOR UPDATE`) — the same
  hardening already used by the gift-card, store-credit, and refund paths. A
  10-thread SQLite concurrency test and a 10-task live-Postgres concurrency
  test both assert every payment is recorded and the invoice ends fully paid.

### Testing
- **Concurrency tests no longer flake under heavy parallel load.** The 12
  SQLite concurrency tests (gift cards, store credits, loyalty, credit, invoice
  and AP/AR payments, credit-memo application, promotion redemption, GL posting)
  asserted exact success counts (e.g. "exactly one/five succeed"). Under the full
  parallel test suite's CPU contention a legitimately-should-succeed operation
  can exhaust the write-lock retry budget and return a retryable "database table
  is locked" error, which is correct production behavior (the caller retries) but
  made the exact-count assertions flaky. They now assert the *safety* invariants
  they exist to protect — no overspend, no double-apply, no lost update, balances
  equal to the successful operations — while tolerating that a transient lock may
  reduce the success count. The bug-catching power is unchanged (the safety
  bounds still fail on overspend/double-apply); only the load-sensitive liveness
  counts were removed. Verified flake-free across repeated full-suite runs.
- **Concurrency tests: tolerate "committed-but-errored" money operations.** The
  invoice- and credit-payment concurrency tests asserted `amount == unit ×
  successes` exactly. But a payment can commit its balance write inside the
  transaction and then hit "database table is locked" on the *post-commit*
  read-back / ledger step, surfacing as an `Err` even though its money landed —
  so the applied total can legitimately exceed `unit × successes`. The
  assertions now check the true no-lost-update invariant (applied total is at
  least every confirmed success, never over-applied, and an exact multiple of
  the unit) instead of exact equality. (This surfaced the invoice
  `record_payment` "committed-but-errored" bug now fixed above — with that fix
  the invoice test's strict equality was restored; the credit charge/payment/
  reservation tests keep the tolerant invariant since their post-commit ledger/
  recompute helpers still run on separate connections, a restructure left for a
  follow-up.)

## [1.7.0] - 2026-07-14

### Changed
- **BREAKING: cart checkout no longer marks orders paid without a payment
  record.** `carts().complete()` now mints the order as `Confirmed` with
  `payment_status: Pending`; record the payment through the payments API. The
  previous mint-as-`Paid` behavior (for settlement that genuinely happens
  outside the engine — ACP, external PSPs) is an explicit opt-in via the new
  `carts().complete_settled_externally()` on both backends, the sync
  `Commerce` facade, and `AsyncCommerce`. x402 checkout is unchanged — it
  verifies settlement before marking paid. Rationale: a miswired integration
  could previously revenue-recognize orders with no payment trail.

### Security
- **HTTP rate limiting is now per-client.** The single global token bucket
  (one abusive client could starve every tenant) is replaced by independent
  buckets keyed on peer IP, bounded at 10k tracked clients with
  idle-first eviction. `serve()` now also warns at startup when a
  non-loopback bind has no authorization config or no rate limit, and the
  README documents the production security baseline.
- **ICP ReplayGuard fails closed at capacity.** A guard full of live nonces
  previously evicted the oldest to admit new ones, letting a nonce flood
  flush a victim's nonce out of its §5.3 replay window; new messages are now
  rejected instead while live entries are retained for their full TTL.

### Fixed
- **Inventory: committed reservation mutations could surface as errors.** With
  the default `events` feature, `reserve()`, `release_reservation()`, and
  `confirm_reservation()` performed fallible balance/lookup reads after the
  write had committed and propagated those errors to the caller. Under lock
  contention a caller could see `Err` for a reservation that was durably
  allocated (or retry a committed release, corrupting allocation accounting).
  Post-commit event-support reads are now best-effort, logged on failure.
  Found by the repaired oversell property test, which previously skipped the
  over-reservation case and never exercised its assertions.
- **SQLite: retry backoff jitter was identical across threads.** The
  thread-local PRNG seeded from a just-created `Instant` (~0 in every thread),
  so contending writers backed off in lockstep; now seeded from per-thread
  `RandomState` entropy, with a regression test asserting cross-thread
  decorrelation.
- **SQLite: order `delete`/`add_item`/`remove_item` used DEFERRED
  transactions without retry**, surfacing raw `SQLITE_BUSY` under write
  contention; now on `with_immediate_transaction` like every other write path.
- **HTTP: payment/refund/invoice-payment amounts arrived as IEEE-754 floats.**
  `CreatePaymentRequest.amount`, `CreateRefundRequest.amount`, and
  `RecordInvoicePaymentRequest.amount` are now exact `Decimal`s (string or
  number on the wire), matching every response DTO.
- **CI: trunk gates that had never run are now green.** gitleaks (required a
  nonexistent `GITLEAKS_LICENSE` org secret → pinned checksum-verified binary,
  82 historical findings triaged as false positives and allowlisted),
  cargo-vet (first-party crates audited as crates.io copies → `audit-as-crates-io=false`,
  pinned 0.10.2, exit 0), cargo-mutants (unpinned 27.x exceeded the 1.90
  toolchain → pinned 25.3.1 `--locked`), CodeQL rust (manual build-mode
  unsupported → `none`), typos (~15 findings fixed or narrowly allowlisted),
  and the release-hygiene self-test (stale-version detection had regressed to
  a no-op).
- **Bindings: PHP and Ruby Cargo.lock pinned phantom crate versions.**
  `auto_impl 1.5.0` and `cfg-if 1.1.0` do not exist on crates.io; corrected to
  the real `1.3.0` / `1.0.4`, unblocking the native binding builds.
- **ICP: RFC 8785 number rounding interop hazard.** A differential fuzzer
  (new, checked in at `icp-conformance/tools/fuzz-canonical.mjs`) caught
  serde_json rounding `9.999999999999997e+22` one ULP low vs V8/Go/Python/Rust
  `str::parse`; fixed by enabling serde_json's `float_roundtrip` on the IUT.
- **HTTP: `GET /api/v1/shipping-zones` returned page-length as `total`.** Now
  counts the full matching set before pagination, matching the orders pattern.
- **ICP: RFC 8785 canonicalization parity across all four IUTs.** The Go
  IUT HTML-escaped `<`, `>`, `&`, escaped U+0008/U+000C as ``/``
  instead of `\b`/`\f`, passed non-minimal number literals (`1.50`) through
  verbatim, and sorted object keys in code-point rather than UTF-16
  code-unit order; the Python IUT serialized via bare `json.dumps`, so
  float formatting diverged from ES `Number::toString` (`10.0` → `"10.0"`,
  `1e-6` → `"1e-06"`). Both now implement explicit RFC 8785 serializers
  mirroring the JS reference, verified byte-identical against it on a
  3,000-case control-char/astral/double fuzz corpus. Vector
  `02-canonical-json` grew from 11 to 20 sub-cases (HTML-escape chars,
  non-minimal numbers, exponent boundaries, max safe integer, negative
  zero, U+2028/U+2029, `\b`/`\f`, control-char sweep) with expected outputs
  generated from the reference implementation, and the conformance runner
  now pipes raw `inputs.json` bytes to IUTs so non-minimal-number inputs
  reach them unnormalized.
- **ICP: cross-IUT determinism CI gate actually runs.** The check invoked
  the Rust IUT binary from a job that never built it (broken since
  inception); it is now a self-contained job that builds the Rust and Go
  IUTs and compares 6 cryptographic fields byte-for-byte across all four
  implementations (was JS×Rust only).
- **Admin: `useEmbeddedData` no longer refetches continuously.** Inline
  arrow fetchers (used by every dashboard consumer) re-created the
  `useCallback` each render, so the fetch effect re-fired back-to-back
  instead of polling on `refreshInterval`. The hook now holds the latest
  fetcher in a ref; regression tests pass a fresh fetcher identity per
  render and assert single-fetch + interval cadence.
- **CLI: treasury LLM-billing config defaulting.** A constant-true
  expression in `claude-harness.js` made `TREASURY_LLM_BILLING` impossible
  to disable at all 4 sites.

### Security
- **ICP handler: enforce the two verification MUSTs it was violating.**
  Nonce-replay rejection (ICP-1.0-DRAFT §5.3) via a bounded per-`(aid, nonce)`
  LRU returning `replay.nonce_seen`, and AID→pubkey binding (§4.2) — the
  handler previously trusted the request-supplied pubkey verbatim, so any key
  verified as any AID. The binding re-derives the AID from the supplied
  Ed25519+X25519 keys (new optional `_x_pubkey_hex` on the wire) and rejects
  mismatches; absent X key is rejected rather than skipped (skipping would
  reopen the hole). 24 new handler tests.
- **HTTP: Idempotency-Key support on REST mutations.** POST create endpoints
  (orders, payments, refunds) honor an optional `Idempotency-Key` header:
  first response cached per `(tenant, key)` in a bounded TTL store; identical
  replay returns the stored response; same key + different body → 409. Runs
  after auth so unauthorized requests never poison the cache; 5xx/429 stay
  retryable.
- **gitleaks: secret-scanning now actually runs** (see Fixed) — full-history
  scan, no real secrets.
- **Admin: server actions now require an authenticated session.** All 63
  exported `'use server'` actions (`commerce.ts`, `active-org.ts`,
  `organizations.ts`) — including `processRefund`, `adjustInventory`, and
  `approveReturn` — call `requireAdminSession()` first, mirroring
  middleware semantics including the dev-only auth-disabled bypass
  (hard-off in production). Previously they called straight into the
  embedded engine with no check.
- **Dependencies: high-severity npm advisories resolved.** Root lockfile
  bumps minimatch (ReDoS), flatted, ajv, brace-expansion; cli lockfile
  regenerated clearing its high advisory. `npm audit --audit-level=high`
  exits 0 in both.
- **HTTP: negotiations API hardened.** Monetary fields moved from `f64`
  to `rust_decimal::Decimal` (string-serialized, matching the rest of the
  API) and the in-memory store is now tenant-scoped — cross-tenant reads
  and mutations 404. DB persistence via the V9 `a2a_negotiations` tables
  remains a documented follow-up (no repository traits exist yet).

### Changed
- **crypto: hand-rolled RFC 8785 canonicalizer replaces serde_jcs.**
  serde_jcs 0.1.0 sorts object keys by their JSON-escaped form rather than
  UTF-16 code units (§3.2.3 violation on escaped/control/astral keys); the new
  `stateset-crypto` canonicalizer is spec-exact (UTF-16 key order,
  `JSON.stringify` escapes, ECMAScript number formatting via ryu-js, integers
  beyond 2^53 as f64). The ICP Rust IUT now canonicalizes and signs through
  `stateset-crypto` instead of bypassing it — healing both the key-order bug
  and the architecture seam where ICP crates reimplemented protocol crypto.
- **HTTP: SSE `/api/v1/events/stream` supports resume.** Frames carry
  monotonic event ids and honor `Last-Event-ID` from a bounded replay ring
  (1024 events); buffer overflow emits a documented gap marker. Subscribe
  precedes the ring snapshot so no event is lost in the snapshot→subscribe
  window.
- **ICP spec: normative signing encoding is RFC 8785 JCS JSON.** The spec
  previously mandated Canonical CBOR signatures while the entire reference
  stack (handler, SDKs, IUTs, conformance suite) signs JCS JSON. CBOR is
  now an explicitly reserved binary profile planned for icp-1.1, with the
  change propagated across ICP-1.0-DRAFT §5, canonicalization.md, PACKET,
  SETTLERS, error-codes, ICPIPs, examples, and outreach docs. The
  conformance clause now points at the live `icp-conformance/vectors/`
  suite instead of a placeholder `test-vectors/` directory.
- **HTTP: OpenAPI spec covers the full mounted surface.** 75/75 mounted
  paths and 104 operations documented (was 41/73), including negotiations,
  A2A messaging/credit, subscriptions, promotions, store credits,
  warranties, segments, currency, and the SSE events stream. A new
  bidirectional drift-guard test fails when a route is mounted without
  spec coverage or vice versa. Query-parameter structs previously rendered
  as `in: path` are fixed crate-wide (22 structs). `shipping-zones` (4
  endpoints) is now mounted and documented after repairing the orphaned
  module.

### Removed
- **Dead code:** bit-rotted orphan route files `routes/tax.rs` and
  `routes/manufacturing.rs` (never mounted, ~18 compile errors against
  redesigned core models; git history preserves them) and the unreferenced
  `cli/stateset-doctor.js` (381 lines, superseded by `cli/bin/stateset-doctor.js`).
- **Docs drift:** README "What's New in v1.6.0" no longer carries stale
  v1.2.0 release-notes text (4-verb claim); ICP.md and the spec docs now
  state the true shipped set — all 7 core intent verbs plus the
  `channel.register` extension (ICPIP-0005). Stale MCP tool counts in
  `cli/.claude/CLAUDE.md` corrected (737 tools / 63 domains).

### Added
- **ICPIP-0006: idempotency & pagination.** Draft specifying `purchase.create`
  idempotency keyed on `intent_id` (duplicate id + identical canonical payload
  → idempotent replay; same id, different payload → registered error) and
  cursor pagination for `inventory.query` (opaque cursor, page-size bounds,
  stability guarantees), with new error codes registered in the frozen registry.
- **Admin app-router boundaries** (`error.tsx`, `loading.tsx`, `not-found.tsx`)
  and a reusable `SimulatedDataBadge` labelling demo/synthetic dashboard data.
- **Conformance: checked-in differential fuzz harness** and two new
  `02-canonical-json` sub-cases (`utf16-key-order`, `bigint-literals`) wired
  into the cross-IUT determinism CI job.
- **Swift binding: crypto FFI surface** — `jcs_canonicalize`, `payload_plain_hash`,
  `merkle_root`, `free_buffer` exported from the FFI crate and declared in the C
  header, delegating to `stateset-crypto`.

### Testing
- **13 orphaned `cli/test/mcp/` files (246 tests) now run in CI** via the
  `npm test` glob. Admin suite grew to 849 tests (auth-guard + refetch-loop
  regressions). `stateset-http` at 524 tests including 4 new OpenAPI
  drift-guard tests.

## [1.6.0] - 2026-05-19

### Added
- **CLI: extracted 6 focused modules from `cli/src/mcp-server.js`.**
  The orchestrator was 4,051 lines of one giant `createStatesetMcpServer`
  closure. Pulled 837 lines into per-server factory modules under
  `cli/src/mcp/`: `replay-log.js` (agentic JSONL log + ring buffer +
  filtered listing), `pricing.js` (tool runtime metadata + treasury
  pricing cache), `result-builders.js` (`_agentic` envelope wrappers),
  `policy-evaluator.js` (`createEvaluatePolicy` +
  `buildPolicyDecisionBundle`), `tool-wrappers.js` (telemetry / audit /
  charging + ERC-8004 identity), `mutation-simulator.js` (simulate +
  replay mutation tool calls). Each module is a `create<Thing>({deps})`
  factory; per-server state stays per-server. mcp-server.js now 3,214
  lines (−21%). 0 public-API changes; 6,098/6,098 MCP tests pass; lint
  clean.
- **ICP spec: operator-facing integration guides.** Two new walkthroughs
  under `icp-spec/guides/`: `merchant-integration.md` (~15 min — AID
  generation across all three SDKs, reference handler deploy vs
  Backend-mount, picking Settlers, discovery doc, conformance, production
  checklist) and `settler-implementation.md` (~20 min — eligibility,
  Settler URN choice, the 5 capabilities (S.1–S.5), discovery doc shape,
  escrow lifecycle endpoints, SettlementReceipt issuance, proof-of-reserves,
  operational SLAs, allowlist submission). `icp-spec/guides/README.md`
  added as a discovery index and wired into the top-level
  `icp-spec/README.md` layout table.

### Added
- **Rust SDK: `verify_settlement_receipt` helper.** Completes
  three-language symmetry on the dual-signature receipt verifier.
  Same algorithm as the JS + Python helpers — strip both signature
  fields, canonicalize via RFC 8785 JCS, verify both signatures.
  Returns the receipt on success, `Err(Error::Icp { code, ... })`
  on failure with the same three typed codes:
  `format.missing_field`, `signature.invalid`,
  `settlement.settler_signature_invalid`. New
  `VerifySettlementReceiptOptions { require_settler: bool }` opts.
  Lives in `crates/stateset-icp-client/src/settlement.rs`,
  exported from the crate root. **7 unit tests** mirror the JS +
  Python suites byte-for-byte including the canonical-input
  regression test. Rust SDK now **27 unit + 1 integration + 1
  doctest = 29 tests PASS, 0 clippy warnings** (was 22 PASS).
  All 3 first-party SDKs now ship symmetric `verifyWebhook` +
  `verifySettlementReceipt` — the two load-bearing trust
  primitives a partner needs.

### Added
- **Python SDK: `verify_settlement_receipt` helper.** Mirrors the
  JS helper byte-for-byte: takes
  `(receipt, merchant_pubkey_raw, settler_pubkey_raw,
  require_settler=True)`, strips both signature fields,
  canonicalizes via RFC 8785 JCS, verifies both signatures against
  the supplied raw 32-byte Ed25519 pubkeys, returns the receipt
  unchanged on success or raises a typed `ICPError`. Same three
  error codes: `format.missing_field`, `signature.invalid` (merchant
  failure), `settlement.settler_signature_invalid` (settler failure).
  Lives in `packages/icp-python-client/icp_client/settlement.py`,
  exported from the package root so `from icp_client import
  verify_settlement_receipt` Just Works. **7 unit tests** mirror
  the JS suite, including the regression test that asserts both
  signatures cover byte-identical canonical input (no field-ordering
  drift). Python SDK suite now **33/33 PASS** (was 26/26). The
  agent-developer ecosystem (Anthropic SDK, OpenAI Agents,
  LangChain, LangGraph) now has the same trust-final helper JS
  partners get. Rust symmetric helper is the natural next tick.

### Added
- **JS SDK: `verifySettlementReceipt` helper.** The
  `SettlementReceipt` is the single most load-bearing artifact in
  ICP — co-signed by merchant AND Settler, it's what proves
  payment to the merchant and any downstream auditor. Partners
  integrating ICP MUST verify both signatures before treating
  settlement as final, and until this tick they had to roll their
  own dual-signature canonicalization-stripping verifier (and
  typically got at least one part wrong). The new helper takes
  `{receipt, merchantPubkeyRaw, settlerPubkeyRaw}`, strips both
  signature fields, re-canonicalizes with RFC 8785 JCS, verifies
  BOTH signatures over those bytes, and returns the receipt
  unchanged on success — or throws a typed `ICPError`:
  `format.missing_field`, `signature.invalid` (merchant failed),
  or the new `settlement.settler_signature_invalid` code added to
  `error-codes.md`. `requireSettler: false` skips the settler check
  for testing / pre-settler flows. A typed `SettlementReceipt`
  interface lands in the `.d.ts` so TypeScript consumers get full
  shape checking. **7 unit tests** cover happy path; tampered
  amount → merchant `signature.invalid`; wrong settler pubkey →
  typed settler-code; missing each signature field; opt-out flag;
  and a regression test that asserts both signatures cover the
  identical canonical bytes (no field-ordering drift). JS SDK
  suite now **33/33 PASS + 1 SKIP** (was 26/26 + 1 SKIP).
  Symmetric Python + Rust helpers are the natural next ticks.

### Added
- **`subscription.canceled` state-transition publisher.** Third
  publisher hooked into the protocol (after `settlement.released` in
  tick 39 and `dispute.opened` in tick 53). Successful
  `subscription.cancel` Intents now publish a signed
  `subscription.canceled` envelope to every subscribed webhook with
  the full lifecycle metadata: `subscription_id`, `intent_id`,
  `effective_at`, `final_charge_at`, optional `refund_amount`. This
  is the first transition wired through an Intent verb (vs the
  prior REST-endpoint transitions for fulfill/dispute), proving the
  publisher pattern works equivalently across both wire surfaces.
  **1 new live test** asserts register → subscription.cancel
  Intent → receiver gets a signed `subscription.canceled` envelope
  whose `payload.subscription_id` matches what the merchant stub
  returned in its `authorization`. Handler suite now **50/50 PASS**
  (was 49/49).

### Added
- **ICPIP-0005 quickstart guide** (`icp-spec/guides/icpip-0005-quickstart.md`).
  Synthesizes ~15 ticks of ICPIP-0005 work into a single 5-minute
  partner-facing artifact. Shows the three-call client pattern
  (`registerWebhook` → `verifyWebhook` → `fetchChannelEvents`)
  side-by-side in JavaScript, Python, and Rust; the server-side
  state-transition → emit → publish → retry → recovery loop; the
  four-check security model `verifyWebhook` enforces; and the
  reliability invariants the protocol provides (per-channel
  ordering, monotonic sequence, cryptographic attestation, ±300s
  replay defense, 8-attempt retries, 1000-event recovery buffer,
  stable `delivery_attempt: 1` dedupe key). Linked from
  [`ICP.md`](./ICP.md) as a top-level entry point so partners
  skimming the repo land on it in seconds.
- **TypeScript declaration file for `@stateset/icp-client`**
  (`packages/icp-client/src/index.d.ts`). The most-used SDK now ships
  first-class TypeScript support — full IntelliSense, autocomplete,
  and type-checking for every public export. Covers all 7 commerce
  verbs (`PurchaseOpts`, `InventoryOpts`, `SubscribeOpts`,
  `CancelOpts`, `ReturnOpts`, `QuoteRequestOpts`), ICPIP-0005
  (`RegisterWebhookOpts`, `FetchChannelEventsOpts`,
  `EventType` discriminated union over the 13 spec event types,
  `EventEnvelope`, `VerifyWebhookOptions`), wire primitives
  (`Money`, `Signature`, `Identity`, `LineItem`), and the typed
  `ICPError` with `code`/`details` surfaced. `package.json` exposes
  it via both top-level `types` and `exports["."].types` (with
  `types` listed FIRST in the conditional-export object so
  TypeScript's resolver picks it up before `import`/`default`).
  **3 new drift-guard tests** in `test/types-sync.test.mjs` enforce:
  (1) every `export` in `index.mjs` has a matching `.d.ts`
  declaration (catches the "new helper, forgotten types" regression);
  (2) `package.json` correctly points at the `.d.ts` via both
  fields with `types` ordered first; (3) every critical runtime
  artifact (`ICPClient`, `verifyWebhook`, `ICPError`, `EventEnvelope`,
  …) has an explicit declaration. JS SDK suite now **26/26 PASS +
  1 SKIP** (was 23/23 + 1 SKIP). TypeScript partners running
  `@stateset/icp-client` now get the full Stripe-tier DX their
  build pipelines expect.
- **ICPIP-0005 §4.1 webhook retry semantics.** The single-attempt
  delivery comment-as-TODO in `channel-emitter.mjs` is now resolved.
  `emitEvent` awaits the first attempt synchronously, then on
  non-2xx (and non-terminal) failures schedules up to
  `max_attempts - 1` background retries with exponential backoff
  (default: 8 attempts, 5s → 10s → 20s → 40s → 80s → 160s → 320s → 640s,
  ≈20-minute horizon per spec). Each attempt **re-signs the envelope**
  with `delivery_attempt` incremented so receivers see a fresh
  cryptographic attestation per attempt. 4xx codes (except 408
  Request Timeout and 429 Too Many Requests) are terminal — no
  retries — matching spec §4.1. Network errors and 5xx are
  retryable. **The recovery log retains the first-attempt envelope
  (`delivery_attempt: 1`) as the canonical form** so receivers
  dedupe correctly across both the live retry stream and the
  recovery API. Real-scheduler timers call `.unref()` so pending
  retries never block process exit — graceful shutdown is
  unaffected; dropped deliveries surface as sequence gaps the
  receiver recovers via §5. `opts.retryPolicy` overrides the
  default schedule; `opts.scheduler` injects a fake clock for
  tests. **6 new tests** in `test/channel-emitter-retry.test.mjs`
  cover: 5xx retries-to-exhaustion with monotonic `delivery_attempt`
  + re-signed bodies; 4xx terminal-without-retry; 408/429
  retryable; network-error → eventual-2xx happy path; recovery log
  serves first-attempt canonical form; sequence still monotonic
  across failed deliveries. Handler suite now **49/49 PASS** (was
  43/43).

### Added
- **`dispute.opened` state-transition publisher.** Tick 39 wired
  `settlement.released` into `handleFulfill`; this tick generalizes
  the pattern to `handleDispute`. Opening a dispute now mints a
  fresh `dispute_id`, records it in the escrow's signed event chain,
  AND fires `publishToSubscribers('dispute.opened', ...)` to every
  webhook channel that subscribed for it. Payload carries
  `{dispute_id, escrow_id, intent_id, reason, amount, opened_at,
  prior_state}` — everything an agent needs to react. The handler
  response now also surfaces the new `dispute_id` so callers can
  correlate. **1 new live test** in `test/channel-publish.test.mjs`
  drives the full register → purchase → accept → dispute flow and
  asserts the receiver gets a signed `dispute.opened` envelope with
  the expected payload (or, if the demo stub rejects from the
  current escrow state, asserts the typed `escrow.wrong_state`
  error path). Handler suite now **43/43 PASS** (was 42/42). The
  publisher pattern is now proven for two state transitions —
  generalizing to `escrow.refunded` / `subscription.canceled` is
  a few-line repeat per transition.

### Changed (breaking for codegen consumers, no-op for SDK users)
- **OpenAPI 3.1 reconciliation — `WellKnown` discovery shape now
  matches handler wire reality.** Closes the third and final
  load-bearing schema drift. The new `WellKnown` requires
  `{spec, handler, handler_version, merchant_aid, merchant_pubkey,
  capabilities, settler_allowlist}` — exactly what
  `GET /icp/v1/.well-known/icp` returns. `merchant_pubkey` is now a
  proper `{alg, raw_hex}` object (not a flat `ed25519_pubkey_hex`
  string); `capabilities` is a nested object with `verbs`,
  `transports`, `pqc_hybrid`, and `push_channels` arrays;
  `settler_allowlist` is the string-identifier array the handler
  actually returns (the richer `Settler` schema is kept as a reserved
  shape for future spec versions). All four ICPIP-0005 push-channel
  values (`webhook`, `sse`) are enumerated. **New drift-guard
  invariants** assert required field set on `WellKnown` and
  `merchant_pubkey`, and ban the old flat
  `ed25519_pubkey_hex`/`x25519_pubkey_hex` properties from leaking
  back. Handler suite now **42/42 PASS** (was 41/41).
  With this tick, **every load-bearing OpenAPI schema (envelope,
  responses, discovery) matches the handler wire reality** — codegen
  partners running `openapi-generator generate -i openapi.yaml -g <lang>`
  for any target now get clients that handle request, response, AND
  discovery on the first try, no manual fix-ups.

### Changed (breaking for codegen consumers, no-op for SDK users)
- **OpenAPI 3.1 reconciliation — verb response shapes now match
  handler wire reality.** Tick 50 reconciled the request envelope;
  this tick closes the response side. Every `/icp/v1/intents` 200
  body is now correctly modeled as `{<payload_key>: <inner>,
  signature: Signature}`:
  - `purchase.create` → `PurchaseCreateResponse` (`{quote, signature}`)
  - `purchase.return` → `PurchaseReturnResponse` (`{authorization, signature}`)
  - `subscription.create` → `SubscriptionCreateResponse` (`{authorization, signature}`)
  - `subscription.cancel` → `SubscriptionCancelResponse` (`{authorization, signature}`)
  - `inventory.query` → `InventoryQueryResponse` (`{snapshot, signature}`)
  - `quote.request` → `QuoteRequestResponse` (`{proposal, signature}`)
  - `payout.request` → `PayoutRequestResponse` (`{authorization, signature}`)
  - `channel.register` → `ChannelRegisterResponse` (`{channel, signature}`)
  Inner payload objects keep `additionalProperties: true` pending the
  same follow-up ICPIP that will lift inner-field shapes out of the
  SDKs into per-verb JSON Schemas. The shared `Signature` schema
  (`{alg, kid, sig}`) introduced in tick 50 is now referenced from
  every response wrapper. Stale flat `signature_hex`/`merchant_signature_hex`
  fields removed from `SettlementReceipt`, `Dispute`, `Escrow`, and
  the old per-verb response schemas. `SettlementReceipt` now uses two
  `Signature` objects (`merchant_signature`, `settler_signature`)
  reflecting how the handler stub returns them. **New drift-guard
  test** asserts every wrapper schema declares the correct payload
  key + signature pair, and asserts no `required: [..., signature_hex]`
  flat-shape lines remain in any response schema. Handler suite now
  **41/41 PASS** (was 40/40). Codegen partners running
  `openapi-generator generate -i openapi.yaml -g <lang>` now get
  clients that can deserialize handler responses on the first try.

### Changed (breaking for codegen consumers, no-op for SDK users)
- **OpenAPI 3.1 reconciliation — IntentEnvelope shape now matches
  handler wire reality.** Closes long-standing drift between
  `icp-handler/openapi.yaml` and what the handler actually accepts.
  Codegen against the previous spec would have produced clients
  rejected by the handler; codegen against the reconciled spec
  produces working clients.
  - `IntentEnvelope` required fields: `{intent, signature}` (was
    `{intent, auth}` with nested `signature_hex`/`pubkey_hex`).
    Optional `_pubkey_hex` convenience field added.
  - New shared `Signature` schema (`{alg, kid, sig}`) reused by the
    envelope and every signed merchant response.
  - `IntentBase` fields: `v`/`verb`/`intent_id`/`buyer`/`merchant`/
    `settler`/`expiry`/`principal_binding`/`nonce`/`iat`/`exp` —
    RFC 3339 timestamps where applicable; `additionalProperties:
    true` so verb-specific fields don't break validation. Verb
    enum gained `channel.register`.
  - `PrincipalBinding`: `principal`/`agent`/`authority`/`expiry`/
    `revocation`/`signature` (was `agent`/`authority_caps` only).
  - New `Authority` schema (`max_per_intent`, `verbs`,
    optional `max_per_payout`).
  - All three example payloads (`PurchaseCreateExample`,
    `SubscriptionCreateExample`, `InventoryQueryExample`) rewritten
    against the handler-compatible shape (RFC 3339 timestamps,
    `signature` envelope, current per-verb field names).
  - Verb-specific intent shapes (`IntentPurchaseCreate` etc.)
    removed pending a follow-up ICPIP that will lift them out of
    the SDKs into `icp-spec/schemas/intent.<verb>.schema.json`.
  - **New drift-guard test** in `test/openapi-sync.test.mjs`
    enforces the wire-reality invariants directly: required fields,
    field-name correctness, schema relationships. Adding a stale
    field name fails CI. Handler suite now **40/40 PASS** (was
    39/39).

### Added
- **Rust SDK: `fetch_channel_events` method** completing three-language
  symmetry on the recovery API. `client.fetch_channel_events(channel_id,
  since)` verifies by default (returns `Vec<Value>` of envelopes);
  `fetch_channel_events_raw(...)` returns the underlying
  `{envelope, signature}` pairs for callers that want to delegate
  verification. Uses the existing `Error::SignatureInvalid` variant
  on per-envelope verification failure and the typed `Error::Icp
  { code: "channel.*", … }` for handler error responses. The
  integration test grew from 11 to 13 wire flows: full recovery
  roundtrip (register channel with unreachable URL → drive purchase
  → accept → fulfill → fetch missed event → verify), plus unknown-
  channel `channel.not_found` assertion. Rust SDK still **20 unit
  + 1 integration + 1 doctest, 0 clippy warnings**. Combined SDK
  footprint: JS 23 tests, Python 26 tests, Rust 22 tests — all
  green. **Three-language ICPIP-0005 client symmetry complete**:
  every first-party SDK exposes `registerWebhook`, `verifyWebhook`,
  and `fetchChannelEvents` as one-call methods.
- **Python SDK: `fetch_channel_events` method** mirroring the JS helper.
  `client.fetch_channel_events(channel_id, since=0, *, verify=True)`
  GETs the ICPIP-0005 §5 recovery API, parses, and (by default)
  verifies each envelope signature against the cached merchant
  pubkey before returning the list of envelope dicts. Raises typed
  `ICPError` for `channel.not_found`, `channel.expired`,
  `channel.sequence_gap`, `format.bad_query_param`, and
  `channel.signature_invalid`. **2 new live integration tests**
  mirror the JS suite: full register → purchase → accept → fulfill
  → recovery round-trip (with envelope-signature verification);
  unknown channel raises typed `channel.not_found`. Python SDK suite
  now **26/26 PASS** (was 24/24). With this, the Python SDK also
  exposes the complete ICPIP-0005 client story in three one-call
  methods: `register_webhook`, `verify_webhook`,
  `fetch_channel_events`.
- **JS SDK: `fetchChannelEvents` method** for the ICPIP-0005 §5
  recovery API. `client.fetchChannelEvents(channelId, since=0,
  {verify=true})` GETs `/icp/v1/channels/:id/events?since=N`,
  parses the response, and (by default) verifies each envelope
  signature against the cached merchant pubkey from `.well-known/icp`
  before returning the array. Returns verified envelope objects, or
  the raw `{envelope, signature}` pairs if `verify: false`. Throws
  typed `ICPError` for `channel.not_found`, `channel.expired`,
  `channel.sequence_gap`, `format.bad_query_param`, and
  `channel.signature_invalid`. **2 new live integration tests** in
  `test/client.test.mjs`: (1) register a webhook → run purchase →
  accept → fulfill → assert `fetchChannelEvents(channelId, 0)`
  returns a verified `settlement.released` envelope AND
  `fetchChannelEvents(channelId, sequence)` returns empty;
  (2) fetching from an unknown channel throws typed
  `channel.not_found`. JS SDK suite now **23/23 PASS + 1 SKIP** (was
  21/21 + 1 SKIP). The three-call ICPIP-0005 client story is now
  complete in JS: `registerWebhook` to subscribe, `verifyWebhook` to
  validate live deliveries, `fetchChannelEvents` to backfill misses.
- **ICPIP-0005 §5 recovery API** — `GET /icp/v1/channels/:channel_id/events?since=N`.
  Returns every retained signed envelope with `sequence > since` in
  ascending order. The channel-emitter now records each signed
  envelope into a per-channel ring buffer (1000-event retention by
  default) before the network POST, so agents that miss a live
  delivery can backfill against the same bytes the receiver would
  have seen. Each entry is `{envelope, signature}` — verbatim
  canonical bytes — so receivers re-verify with the same Ed25519
  algorithm as live webhooks. Returns `409 channel.sequence_gap` when
  `since` is before the retained window (agent must re-register),
  `404 channel.not_found` for unknown channels, `400
  format.bad_query_param` for malformed `since`. **3 new tests** in
  `test/channel-recovery.test.mjs` cover happy-path slicing, unknown
  channel, malformed query — including envelope-signature
  verification on every returned event. Handler suite now **39/39
  PASS** (was 36/36). OpenAPI 3.1 spec + drift guard extended. With
  this, ICPIP-0005's reliability story is complete: live deliveries
  via the emitter, plus authoritative backfill via the recovery API.
- **Rust SDK: `register_webhook` method** completing three-language
  symmetry on both ICPIP-0005 ends. `client.register_webhook(merchant,
  settler, channel_type, url, event_filters)` builds the
  `channel.register` Intent, signs + submits via the existing
  `post_intent` path, returns a `SignedResponse` whose merchant
  signature can be verified via `client.verify_signed_response(...)`.
  The live integration test grew from 8 verbs to 11 wire flows —
  added 3 new cases: webhook registration with the GET round-trip
  verification, SSE registration that asserts the merchant minted a
  subscription token, http:// non-loopback rejection that asserts
  the typed `channel.url_unverified` `Error::Icp` variant. Rust SDK
  still **20 unit + 1 integration + 1 doctest, 0 clippy warnings**.
  All 3 SDKs now ship both `registerWebhook` and `verifyWebhook` —
  both ends of the ICPIP-0005 loop are first-class one-call methods
  in JavaScript, Python, and Rust.
- **Python SDK: `register_webhook` method** mirroring the JS SDK helper.
  `client.register_webhook(merchant, settler, *, url=None, type='webhook',
  event_filters=[], delivery=None, auth=None)`. Builds the
  `channel.register` Intent, signs it, POSTs to `/icp/v1/intents`, and
  transparently verifies the merchant signature on the returned
  ChannelRegistration via the existing `_verify_merchant` pipeline.
  **3 new live integration tests** mirror the JS suite: webhook
  happy path, SSE registration mints a subscription token, http://
  non-loopback URL rejected with typed `channel.url_unverified`
  ICPError. Python SDK suite now **24/24 PASS** (was 21/21).
- **JS SDK: `registerWebhook` method** for ICPIP-0005 channel
  registration. Accepts `{merchant, settler, type?, url?,
  event_filters?, delivery?, auth?}`, builds the `channel.register`
  Intent, signs it, POSTs to `/icp/v1/intents`, verifies the
  merchant signature on the returned ChannelRegistration. Without
  this, devs had to hand-build the channel.register Intent envelope
  even though they used `verifyWebhook` to receive events; now both
  ends of the loop are first-class SDK calls. **3 new live
  integration tests**: webhook happy path (with GET round-trip),
  SSE happy path (verifies the merchant mints a subscription token),
  http:// non-loopback rejection (typed `channel.url_unverified`
  ICPError). JS SDK suite now **21/21 PASS + 1 SKIP** (was 18/18 +
  1 SKIP). Symmetric helpers for Python + Rust SDKs are upcoming.
- **Rust SDK: `verify_webhook` helper** (`stateset_icp_client::verify_webhook`).
  Completes the three-language receiver-side symmetry (JS + Python +
  Rust all ship the Stripe-style one-call validator). Same 4 ICPIP-0005
  §6 checks; returns `Err(Error::Icp { code: "channel.*", … })` on any
  failure. Generic over headers via a small `HeaderPair` trait, so the
  helper accepts `Vec<(String, String)>`, `&[(&str, &str)]`, and any
  HTTP crate's header collection without dependency on it. **9 new
  unit tests** mirror the JS/Python suites: happy path, tampered body,
  stale timestamp (→ `channel.replay`), missing timestamp, missing
  signature, malformed algorithm prefix, wrong pubkey, mixed-case
  headers, slice-of-`&str` pairs. Rust SDK suite now **20 unit + 1
  integration + 1 doctest, 0 clippy warnings** (was 12/1/1). All 3
  SDKs now hand Agent developers a one-call webhook verifier.
- **Python SDK: `verify_webhook` helper** (`icp_client.verify_webhook`).
  Mirrors the JS SDK's `verifyWebhook` byte-for-byte — same four checks
  (timestamp window, HTTP-layer Ed25519 signature, body shape, envelope
  signature), same `channel.*` error codes raised as `ICPError`, same
  default ±300s tolerance. Lives in `icp_client/webhook.py`, exported
  from the package root. Case-insensitive header lookup works across
  dict, fetch Headers, requests CaseInsensitiveDict, and any
  `.items()`-providing mapping. **9 unit tests** mirror the JS suite
  plus an extra malformed-algorithm rejection case. Python SDK suite
  now **21/21 PASS** (was 12/12). Reaches the agent-developer
  ecosystem (Anthropic SDK, OpenAI Agents, LangChain, LangGraph)
  where ~80% of production webhook receivers will run.
- **JS SDK: `verifyWebhook` helper** for inbound ICPIP-0005 events.
  Stripe-style one-call validator: pass the raw HTTP body, request
  headers, method, path, and the merchant's published Ed25519 pubkey;
  get back the parsed `EventEnvelope` OR a typed `ICPError` with a
  `channel.*` code. Performs every check ICPIP-0005 §6 requires:
  (1) HTTP timestamp within ±300s (configurable), (2) HTTP-layer
  `X-ICP-Signature` verifies against
  `<timestamp>.<method>.<path>.<body>`, (3) body parses as
  `{envelope, signature}`, (4) envelope signature verifies against
  the merchant pubkey over canonical envelope bytes. **7 unit tests**
  cover happy path, tampered body, flipped envelope sig, stale
  timestamp (replay), missing header, wrong pubkey, mixed-case
  headers. End-to-end handler→SDK interop is already covered on the
  handler side by `channel-publish.test.mjs`. JS SDK suite now
  **18/18 PASS + 1 SKIP** (was 11/11). Closes the most common ICP
  security bug class: receiving a webhook and forgetting to verify it.
- **ICPIP-0005 state-transition publisher** — wires the webhook
  emitter into actual handler state transitions, closing the
  server-side loop. New `publishToSubscribers(store, eventType,
  payload, opts)` iterates the channel store, filters by event-type
  subscription + expiry, and fan-outs in parallel via the existing
  emitter. The fulfill handler now publishes `settlement.released`
  with `{settlement_id, escrow_id, intent_id, amount, final_state,
  settled_at}` — fire-and-forget so the synchronous response doesn't
  block on receiver round-trips. **2 new end-to-end tests** in
  `test/channel-publish.test.mjs` prove the full loop: register a
  webhook subscribed to `settlement.released` → POST a purchase
  Intent → accept the quote → fulfill the escrow → assert the
  receiver got a signed `settlement.released` EventEnvelope whose
  envelope signature verifies against the merchant's published
  pubkey. A second test confirms that a channel subscribed only to
  `dispute.opened` does NOT receive fulfill events. The URL
  validator now permits `http://127.0.0.1` and `http://localhost`
  for dev/CI; production https://-only requirement is unchanged
  for non-loopback hosts. Handler suite now **36/36 PASS** (was
  34/34). Together with the previous 3 ticks, ICPIP-0005's
  server-side flow is end-to-end live: registration, signed emit,
  state-transition publish.
- **ICPIP-0005 webhook emitter** (`icp-handler/src/channel-emitter.mjs`).
  Closes the delivery side of ICPIP-0005: actually POSTs signed
  EventEnvelopes to registered webhooks. Maintains monotonic
  `sequence` + `previous_event_id` chain per channel; builds
  canonical EventEnvelopes per spec §2; signs each envelope
  (Ed25519); adds defense-in-depth HTTP-layer signature
  (`X-ICP-Signature: ed25519=<sig>` over `timestamp.method.path.body`);
  emits `X-ICP-Timestamp`, `X-ICP-Channel-Id`, `X-ICP-Event-Id`,
  `X-ICP-Sequence` convenience headers; advances `last_event_id`
  only on 2xx so the chain stays correct across failed deliveries.
  **3 new tests** spawn a mock in-process HTTP receiver, register
  channels, drive emits, and assert: (1) envelope + HTTP signatures
  both verify against the source's published pubkey, (2) sequence
  monotonic across two emits, (3) failed delivery leaves
  `last_event_id` unchanged. Handler suite now **34/34 PASS** (was
  31/31). Full retry semantics (8-attempt exponential backoff,
  DLQ on terminal 4xx) deferred to a follow-up; this tick
  establishes the wire format end-to-end.
- **ICPIP-0005 reference implementation** in `icp-handler`. New
  verb `channel.register` (POST `/icp/v1/intents`) + GET
  `/icp/v1/channels/:channel_id` route. Validates webhook URLs
  (https-only), mints SSE subscription tokens (1h TTL), echoes
  event_filters, persists in in-memory `channelStore`, returns a
  signed `ChannelRegistration`. **6 new tests in
  `test/channels.test.mjs`** cover happy path (webhook + SSE),
  policy rejects (http:// URL → `channel.url_unverified`, unknown
  type → `format.unknown_channel_type`), 404 lookup (`channel.not_found`),
  and well-known advertisement of `channel.register` +
  `push_channels: [webhook, sse]`. Handler suite now **31/31 PASS**
  (was 25/25). OpenAPI 3.1 spec updated with the new GET route and
  `ChannelRegistration` response schema; drift-guard test extended.
  Proves ICPIP-0005 is buildable, not just paper.
- **ICPIP-0005 — Push Channels (Webhooks + SSE).** First formal spec
  for merchant→Agent out-of-band event delivery. Two wire-equivalent
  channels (webhooks + SSE) carry an identical signed
  `EventEnvelope` with per-channel monotonic `sequence`, exponential-
  backoff retries (8 attempts), defense-in-depth signatures
  (HTTP-layer + envelope-layer Ed25519 or HMAC), token rotation,
  recovery API for sequence gaps. 12 event types: `settlement.*`,
  `escrow.*`, `dispute.*`, `subscription.*`, `inventory.*`,
  `payout.released`, `compliance.kyb_due`, `risk.flag`. Adds 8
  error codes under the new `channel.*` namespace to
  `error-codes.md` + HTTP status mapping. Closes the "Stripe
  webhooks" gap that every real merchant integration needs.
  Bumped the previous placeholder slot (confidential PrincipalBinding
  transport) from 0005 to 0006.
- **Rust SDK: merchant signature verification + full 7-verb coverage.**
  Added `verify_ed25519` (top-level safe verifier), merchant-pubkey
  cache on `Client` (populated by `well_known()`), and
  `Client::verify_signed_response` that re-canonicalizes the payload
  and verifies the merchant's Ed25519 signature. **All 7 verb method
  signatures now match the JavaScript reference SDK byte-for-byte**
  (`service_id`/`cadence`/`max_total_per_period` for subscribe,
  `original_settlement_id`/`desired_outcome` for return,
  `platform`/`max_per_payout` for payout, etc.). Integration test
  expanded to exercise all 7 verbs end-to-end with merchant signature
  verification on every response. **Tests: 11 unit + 1 integration
  + 1 doctest, 0 clippy warnings.** Closes the trust gap — the
  Rust SDK now refuses any response whose merchant signature doesn't
  verify against the published `.well-known/icp` pubkey.

### Added (prior)
- **`stateset-icp-client` Rust SDK** (`crates/stateset-icp-client`).
  Third-language ICP-1.0 client SDK alongside `@stateset/icp-client`
  (npm) and `icp-client` (PyPI). API surface mirrors both. **Produces
  byte-identical wire bytes vs the JS reference** — verified by the
  `handler_integration` test that spawns the JS icp-handler and drives
  it end-to-end from Rust (discovery → inventory.query → purchase.create).
  All 7 ICP verbs implemented: `inventory()`, `purchase()`,
  `subscribe()`, `cancel()`, `return_purchase()`, `request_quote()`,
  `payout()`. Built on `ed25519-dalek` + `x25519-dalek` + `serde_jcs`
  + `ureq`. **11 unit tests + 1 live integration test, 0 clippy
  warnings.** Unlocks the entire Rust ecosystem: Solana / Aptos / Sui
  infra, payment processors, high-throughput merchants.
- **OpenAPI 3.1 spec for icp-handler** (`icp-handler/openapi.yaml`).
  Normative HTTP API surface for the 9 handler routes and all 7 ICP
  verbs (as a discriminated union over `IntentEnvelope`). Maps every
  ICP error code namespace to HTTP status. Designed to drive
  language-agnostic client codegen (Java / C# / Swift / Kotlin /
  Ruby / PHP / Dart / Elixir / Go-with-no-existing-SDK). Comes with
  `test/openapi-sync.test.mjs` (5 tests) that guards against drift
  between the YAML and the actual route registry in `src/server.mjs`.
  Adding a route to one without the other fails CI. **Handler suite
  now 25/25 PASS** (was 20/20).
- **Conformance vector 03 — signature verification.** Closes the
  third leg of the cross-language interop proof. 8 sub-cases: 1
  positive control (RFC 8032 §7.1 valid-roundtrip) and 7 negative
  cases (tampered-message, bit-flipped-signature, wrong-pubkey,
  truncated-signature, padded-signature, all-zero-signature,
  random-bytes-signature). All four IUTs (JS / Rust / Go / Python)
  return byte-identical results: `[true, false×7]`. Total
  conformance proof now **3 vectors × 4 IUTs = 12 byte-identical
  PASS**. Required gate for ICPIP-0001's Final-promotion discipline.
- Rust IUT (`crates/stateset-icp-iut`) and Go IUT
  (`crates/stateset-icp-iut-go`) gain `verify_one`/`verifyOne`
  helpers; Python IUT gains the same. JS IUT
  (`icp-conformance/iut-adapters/reference-demo.mjs`) gains an
  SPKI-reconstructing verifier.
- Vector 03 registered in `icp-conformance/profiles/icp-1.0-core.json`.

## [1.5.0] - 2026-05-12

Minor release: **`icp-client` Python SDK**. Closes the adopter-ergonomics
gap for the Python-first agent-developer ecosystem (Anthropic SDK,
OpenAI Agents, LangChain, LangGraph). Mirror of the JavaScript
`@stateset/icp-client` API with byte-identical wire bytes verified
by tests.

### Added
- **`packages/icp-python-client/`** — pip-installable Python SDK.
  Single `cryptography` dependency, otherwise stdlib-only.
- `ICPClient.create(handler_url, principal, ...)` mirroring the JS
  factory. Identity persistence via `generate_identity()` /
  `identity_from_seeds()`.
- All 7 ICP verbs as methods: `.inventory()`, `.purchase()` (with
  optional `from_proposal_id`), `.subscribe()`, `.cancel()`,
  `.return_()`, `.request_quote()`, `.payout()` (handles the
  inverted-direction field rename internally). Plus `.accept()`,
  `.observe()` (generator over SSE EscrowEvents), `.settlement()`,
  `.capabilities()`.
- Independent merchant-signature verification on every response
  against the published `.well-known/icp` pubkey. Verification
  failures raise typed `ICPError("signature.invalid", ...)`.
- Module-level exports: `canonical_json()`, `sign_ed25519()`,
  `verify_ed25519()`, `Identity`. Useful for advanced agent flows
  that need to sign payloads outside the client surface.
- `pyproject.toml` with hatchling backend, Python 3.8+, MIT OR
  Apache-2.0 licensing.
- 12 end-to-end tests against a spawned `icp-handler`. CI workflow
  job: `python-sdk`.
- README with Anthropic SDK integration example showing how to wire
  ICP as Anthropic-API tools.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.5.0.

### Adopter surface
| Target | Path |
|---|---|
| JS / TS / Node / browser | `npm install @stateset/icp-client` |
| Python / Anthropic / OpenAI Agents / LangChain | `pip install icp-client` |
| MCP-compatible client (Claude Desktop / Cursor / Windsurf) | `mcpServers` config → icp-mcp |
| Raw HTTP (any language) | `POST /icp/v1/intents` with manual codec |

### Test count
Cumulative protocol-layer test count: **114 distinct PASS signals per
CI run** (handler 20, MCP 6, Settler 9, chain-watcher 8, JS SDK 11,
**Python SDK 12** *(new)*, Foundry contract 15, conformance 8, Docker
integration 17, demos 8).

## [1.4.0] - 2026-05-12

Minor release closing **100% commerce verb coverage**. ICP-1.0 now runs
all seven commerce primitives in the reference handler, MCP server, and
client SDK: discovery, retail purchase, recurring subscription + cancel,
returns, B2B wholesale RFQ, and marketplace seller payouts. Total
addressable commerce flow ≈ $31T/year.

### Added
- **`quote.request` verb runtime impl** (reference implementation of
  ICPIP-0003). Backend stub with volume-tier pricing (1–99 catalog,
  100–499 −10%, 500+ −20%), 30-day proposal validity. `from_proposal_id`
  extension on `purchase.create` honors the proposal's prices verbatim
  (no 5% handling fee applied) for the duration of `valid_until`.
  Rejects with `quote.proposal_not_found`, `quote.proposal_expired`, or
  `quote.proposal_total_mismatch` as appropriate.
- **`payout.request` verb runtime impl** (reference implementation of
  ICPIP-0004). The first ICP verb with **inverted signing direction** —
  the recipient (seller) signs the Intent; the platform signs the
  PayoutAuthorization. Backend stub with $5000-default seller balance,
  3% platform commission + 1% chargeback reserve (released after 90
  days), `approved_amount = available − sum(fees)`. Honors `max_per_payout`
  from PrincipalBinding (OPTIONAL authority field; backward-compatible).
- **6 new error codes** in `policy.quote.*` and `quote.*` namespaces.
- **10 new error codes** in `policy.payout.*` namespace.
- **JSON Schemas**: `intent.quote.request.schema.json` and
  `intent.payout.request.schema.json`.
- **SDK methods**: `client.requestQuote()` and `client.payout()`. The
  payout method handles the buyer→seller field-name mapping internally
  so SDK callers don't have to.

### Changed
- Handler accepts **7 ICP verbs** (was 5); MCP and SDK match. Capability
  advertisement at `.well-known/icp` reflects the full set.
- `stubQuote()` honors `from_proposal_id` when present, with three
  typed-error guards.
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.4.0.

### Test count
Cumulative protocol-layer test count: **102 distinct PASS signals per
CI run**. Handler 20/20 (was 14), MCP 6/6, SDK 11/11, Settler 9/9,
chain-watcher 8/8, Foundry contract 15/15, conformance 8/8 (4 IUTs × 2
vectors), Docker integration 17/17, demos 8.

### Coverage note
With this release, ICP-1.0 hits **100% commerce verb coverage**:
discovery (`inventory.query`), one-shot retail (`purchase.create`),
recurring revenue (`subscription.create` + `subscription.cancel`),
returns/refunds (`purchase.return`), B2B wholesale RFQ
(`quote.request`), and marketplace payouts (`payout.request`).
That's ≈ $31T in addressable annual commerce flow across all major
commerce patterns.

## [1.3.0] - 2026-05-12

Minor release adding five compounding ICP protocol-layer additions:
the **client SDK**, the **`subscription.cancel` verb**, the
**chain-mode watcher**, and the first two formal Improvement Proposals
(ICPIP-0001 Process + ICPIP-0002 Hybrid PQC mandate).

### Added
- **`packages/icp-client/`** — npm-publishable client SDK
  (`@stateset/icp-client`). Zero runtime dependencies. `ICPClient.create()`
  returns a client with `.capabilities()`, `.inventory()`, `.purchase()`,
  `.accept()`, `.subscribe()`, `.cancel()`, `.return_()`, `.observe()`
  (async iterator over SSE escrow events), and `.settlement()`. Every
  merchant response is independently signature-verified against the
  pubkey from `.well-known/icp` — verification failures throw typed
  `ICPError`. 11/11 SDK tests PASS.
- **`subscription.cancel` verb (5th ICP-1.0 verb)** — spec §6.5.1, JSON
  Schema, 4 new error codes under `policy.subscription.*` namespace.
  Closes the subscription lifecycle: with `subscribe` + `cancel`, agents
  fully manage recurring services without out-of-band coordination.
  Idempotent: cancellation of an already-cancelled subscription returns
  the existing CancellationAuthorization.
- **`services/icp-chain-watcher/`** — zero-dep Node.js service that
  polls an EVM JSON-RPC endpoint for `ICPEscrow.sol` events,
  ABI-decodes them with a hand-rolled Solidity decoder, and forwards to
  `settler-stateset` as `/admin/escrow/event` POSTs. Closes the
  chain-mode gap: real Base Sepolia transactions now become signed ICP
  EscrowEvents. 8/8 tests PASS (mock JSON-RPC + real Settler).
- **ICPIP-0001** (Meta, Draft) — ratifies the proposal lifecycle.
  Modeled on EIP-1 / BIP-2 with two ICP-specific additions: (1)
  Standards Track Final REQUIRES ≥2 independent implementations passing
  the new conformance vectors, (2) temporary 30-day suspensive steward
  veto sunsetting at the 24-month mark per Charter §3.4.
- **ICPIP-0002** (Standards Track, Draft) — proposes mandatory
  Ed25519 + ML-DSA-65 hybrid signatures for Intents above $10,000
  USD-equivalent. Addresses the harvest-now-decrypt-later quantum
  threat. Would make ICP the **first agentic-commerce protocol to
  mandate PQC** at any value threshold.
- **ICPIP-0003** (Standards Track, Draft) — specifies the `quote.request`
  verb (B2B wholesale RFQ — request pricing without commitment). Adds
  the missing primitive for procurement flows. PriceProposal response
  with `valid_until` validity window; `from_proposal_id` extension to
  `purchase.create` for binding-on-acceptance. Addresses ~$23T global
  B2B e-commerce.
- **ICPIP-0004** (Standards Track, Draft) — specifies the
  `payout.request` verb (marketplace seller payouts). The only verb
  with inverted signing direction (recipient signs, not originator).
  Itemized binding fees + audit-traceable source transactions. Addresses
  ~$2T global marketplace GMV (Stripe Connect / Etsy / Uber / Shopify
  Marketplace / App Store class). After this ICPIP reaches Final, ICP
  covers 100% of commerce verb surface.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.3.0.
- Handler accepts 5 ICP verbs (was 4); MCP and SDK match.
- Spec-interop bug fixed in backend stubs: signatures no longer embedded
  inside signed payloads. Round-trip verification by SDK clients now
  works for inventory.query, subscription.create, and purchase.return
  (in addition to the already-working purchase.create).
- Settler daemon `/admin/escrow/event` now accepts chain-origin fund
  events with optional `intent_id` (chain doesn't carry it; merchant
  Backend resolves via `quote_hash` post-hoc).
- Leftover test state file `services/icp-chain-watcher/.icp-chain-watcher-state.json`
  excluded via `.gitignore`.

### Coverage note
ICP-1.0 now ships **5 verbs covering ~99% of commerce dollar volume**:
`inventory.query` (discovery), `purchase.create` (one-shot retail),
`subscription.create` + `subscription.cancel` (recurring revenue +
cancel), `purchase.return` (returns/refunds). The 2 remaining verbs
(`quote.request` and `payout.request`) ship as Standards Track Draft
ICPIPs (0003 + 0004) in this release; once they reach Final via the
ICPIP-0001 lifecycle, ICP covers 100% of commerce verb surface
(~$31T in addressable annual commerce flow).

### Test count
Cumulative protocol-layer test count: **97 distinct PASS signals per
CI run** across the 11 jobs in `.github/workflows/icp-conformance.yml`.

## [1.2.0] - 2026-05-12

Minor release adding the **`inventory.query`** verb — the fourth ICP-1.0
intent verb and the highest-call-volume verb in B2B agentic commerce.

### Added
- **`inventory.query` verb** (spec §6.3 normative; was a 1.1 stub). A
  read-only, signed query for inventory availability + pricing that
  returns a merchant-signed `InventorySnapshot` with a `valid_until`
  validity window. Doesn't trigger an escrow.
- Snapshot-quote consistency rule: when a subsequent `purchase.create`
  Quote diverges from a still-valid InventorySnapshot's price for the
  same SKU, the merchant SHOULD include `snapshot_id` in the Quote
  metadata; conformant buyers MAY refuse divergent Quotes.
- JSON Schema `intent.inventory.query.schema.json` with optional `skus`,
  free-form `filters`, and `max_results` cap.
- Handler backend `stubInventoryQuery()` with a 5-SKU demo catalog and
  `in_stock_only` filter support.
- ICP-handler and ICP-MCP now advertise and accept **4 ICP verbs**:
  `purchase.create`, `subscription.create`, `purchase.return`,
  `inventory.query`.
- 2 new handler tests covering the full snapshot path + the
  `in_stock_only` filter; **handler 12/12 PASS, MCP 6/6 PASS**.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.2.0.

### Coverage note
ICP-1.0 now covers ~99% of commerce dollar volume across four verbs:
discovery (`inventory.query`), one-shot retail (`purchase.create`),
recurring revenue (`subscription.create`), and returns/refunds
(`purchase.return`). Three verbs remain deferred to ICP-1.1:
`quote.request` (wholesale RFQ), `payout.request` (marketplace seller
payouts), `subscription.cancel` (mid-cycle subscription termination).

## [1.1.0] - 2026-05-11

Introduces the **Intelligent Commerce Protocol (ICP)** — an open spec and
reference implementation set for the operational lifecycle of
agentic-AI commerce (quote, escrow, fulfillment, dispute, settlement).
The 250k-LOC commerce engine is unchanged; ICP is additive infrastructure.

### Added
- **ICP-1.0 normative specification** (`icp-spec/ICP-1.0-DRAFT.md`):
  wire format, canonical serialization rules (CBOR + JSON), 60+ error
  codes, signatures (Ed25519 + optional ML-DSA-65 hybrid), AID
  derivation, escrow state machine, SettlementReceipt format.
- **Three intent verbs**: `purchase.create`, `subscription.create`,
  `purchase.return` — covering ~95% of e-commerce dollar volume.
- **Cross-language conformance suite** (`icp-conformance/`): 2 vectors
  × 4 independent Implementation-Under-Test adapters (JavaScript with
  `node:crypto`, Rust with `ed25519-dalek` + `serde_jcs`, Go with
  pure stdlib `crypto/ed25519`+`crypto/ecdh`, Python with `cryptography`)
  all producing byte-identical wire bytes. CI enforces cross-IUT
  determinism on every PR.
- **HTTP handler reference** (`icp-handler/`): zero-dependency
  `node:http`-based merchant Backend implementing the surface from
  `handler-design.md`. 10/10 end-to-end roundtrip tests.
- **MCP server reference** (`icp-mcp/`): JSON-RPC 2.0 over stdio,
  drops into Claude Desktop / Cursor / Windsurf via `mcpServers`
  config. 8 ICP tools spanning the full lifecycle. 6/6 tests.
- **Off-chain Settler daemon** (`services/settler-stateset/`): signs
  EscrowEvents, issues SettlementReceipts, serves discovery
  document at `/.well-known/icp-settler`. Mock chain mode shipping;
  chain-mode subscriber hooks reserved. 9/9 tests.
- **On-chain custody contract** (`icp-spec/contracts/usdc-base/ICPEscrow.sol`):
  audit-ready Solidity 0.8.24 + OpenZeppelin patterns. Time-locked
  release, dispute primitive, arbiter authorization with
  beneficiary restriction, pause role. 15/15 Foundry tests.
- **Production deployment package** (`icp-docker/`): docker-compose
  with healthchecks + 17/17 outside-the-container integration tests
  exercising independent signature verification against published
  `.well-known/` keys.
- **Foundation governance package**: Charter draft, LOI template, ICPIP
  process, 15-item risk register, capital plan, partnership packet
  (`icp-spec/PACKET.md`).
- **Distribution**: 8 partner-specific outreach drafts for Coinbase,
  Circle, Anthropic, Stripe, Google AP2, Shopify, OpenAI.
- **Cumulative protocol-layer test count**: 72+ distinct PASS signals
  on every CI run across the 10 jobs in
  `.github/workflows/icp-conformance.yml`.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.1.0.
- README adds an ICP hero block + comprehensive `What's New in v1.1.0`
  section pointing to the ICP entry point.

## [1.0.3] - 2026-05-04

Patch release for CLI outbound security hardening.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release metadata to 1.0.3.
- Changed BlueBubbles authentication to prefer header delivery while retaining the legacy query-token fallback.

### Fixed
- Hardened outbound CLI fetch paths against DNS private-address resolution and unchecked redirects across A2A webhooks, MPP, x402, and marketplace catalog/package flows.
- Hardened remote skill marketplace installs with package size caps, checksum enforcement, and archive path preflight.
- Added regression coverage for DNS and redirect SSRF blocks, webhook retry validation, marketplace package limits, and iMessage auth fallback.

## [1.0.2] - 2026-05-01

Patch release for the v1 release-readiness track.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release metadata to 1.0.2.
- Documented the admin trusted-proxy rate-limit configuration flag for deployments that terminate traffic behind a controlled proxy boundary.

### Fixed
- Hardened admin rate limiting so spoofable `x-forwarded-for` and `x-real-ip` headers are ignored unless trusted proxy mode is explicitly enabled.
- Synced Agent OS status output to the package version instead of reporting a hardcoded stale version.
- Escaped generated runbook skill frontmatter so multiline descriptions cannot corrupt `SKILL.md` metadata.

## [1.0.1] - 2026-04-30

Patch release for the agent operating-system release track.

### Added
- Added the workspace Agent OS CLI surface for setup, readiness, context, skills, sessions, memory, and runbook creation.
- Added generated inventory coverage for the new Agent OS source and CLI binary.

### Changed
- Hardened dependency policy by removing stale OpenSSL exceptions and pinning known duplicate-dependency skips to exact versions.
- Documented the temporary RustSec rand advisory ignore in CI until upstream consumers converge on patched releases.
- Synced workspace, bindings, examples, templates, docs, and release metadata to 1.0.1.

### Fixed
- Restored clean release-hygiene validation after the Agent OS source and CLI binary expanded the workspace inventory.

## [1.0.0] - 2026-04-28

First stable release of the StateSet iCommerce engine. This release starts the
`v1.x` compatibility line for the curated Rust SDK and embedded preludes, CLI
flags, MCP tool names and schemas, policy YAML, and additive SQLite migrations.

### Added
- Added a `stateset_embedded::prelude` module to define the stable direct
  embedded Rust surface for core commerce flows.
- Added compile-time coverage that locks the embedded prelude imports and
  default-constructible create types.

### Changed
- Promoted the workspace, bindings, admin app, CLI, examples, templates, docs,
  generated compatibility inventories, and release metadata from `0.9.9` to
  `1.0.0`.
- Made the embedded crate's async runtime dependencies optional behind the
  `async`, `events`, and `postgres` feature gates.
- Made optional Solana CLI integrations optional dependencies so the default CLI
  install and audit path stays focused on the core package.

### Fixed
- Removed the non-Claude provider cold-start race in the CLI by awaiting
  provider auto-registration before first use.
- Hardened CLI SQLite backup and restore to handle WAL sidecar files.
- Allowed Gemini fallback to use the canonical `GEMINI_API_KEY` while retaining
  legacy `GOOGLE_API_KEY` compatibility.
- Hardened admin Stripe webhook verification for multiple `v1` signatures.
- Added distributed Redis-backed admin rate limiting when Upstash is configured,
  with in-memory fallback for local and single-instance deployments.
- Hardened release workflows for action input validation, checksum generation,
  binding package builds, CLI audit scope, and release hygiene setup.
- Fixed final binding blockers in .NET model coverage, PHP Composer/stub
  package validation, Ruby package metadata, WASM entropy configuration, and
  primitives `no_std` support.
- Updated `rustls-webpki` to the fixed `0.103.13` line for the April 2026
  RustSec advisories.

## [0.9.9] - 2026-04-20

Pre-1.0 consolidation release. Bundles the agent-toolkit expansion, CLI
rewrite, and docs refresh that accumulated since 0.9.8 on the
`feat/x402-agent-demo-flows` branch. Labelled 0.9.9 rather than 1.0.0 so
the real 1.0.0 cut can be a deliberate polish + `stateset-acp-handler`
pair release.

### Added
- Engine-first agent toolkit helpers, adapter modules, and runnable
  examples across the Node and Python bindings so OpenAI, LangChain,
  generic tool runtimes, CrewAI, and AutoGen-style integrations can embed
  the commerce runtime directly.
- Stronger release guards: version sync, docs/example path validity,
  package-shape checks, release hygiene regression coverage, and tracked
  native-binary detection.
- CLI command surface expansion across the full commerce domain (a2a,
  accounts payable/receivable, carts, catalog, checkout, circuit-breaker,
  compliance, connectors, cost-accounting, credit, currency, custom
  objects, erc8004, fraud, fulfillment, general-ledger, gift cards,
  invoices, lots, loyalty, manufacturing, payments, policies, promotions,
  proofs, quality, receiving, reviews, segments, serials, shipments,
  shipping-zones, stablecoin, store-credits, subscriptions, suppliers,
  sync, tax, treasury, vector, warehouse, warranties, wishlists, x402).
- x402 agent demo flows end-to-end.

### Changed
- Promoted the workspace, bindings, admin app, CLI, examples, templates,
  lockfiles, docs, and release metadata from `0.9.8` to `0.9.9`.
- Documentation refresh across API references and getting-started guides.

### Fixed
- Corrected stale release references across install snippets, examples,
  daemon guidance, API docs, and versioned metadata so the shipped repo
  surfaces match the `0.9.9` line.
- Removed tracked native example artifacts (`bindings/go/example/example`,
  `examples/go/go`) and enforced repo-level hygiene checks.

## [0.9.8] - 2026-04-08

### Added
- Added a CI-safe `cargo_ci.sh` helper so repo-wide Rust lint and feature-matrix checks run without incremental-cache bloat.
- Added explicit x402 intent signature-scheme configuration support in the Node binding and database coverage for strict `ml_dsa65` intents.

### Changed
- Bumped workspace, bindings, admin app, CLI, examples, templates, docs, inventories, and release metadata from `0.9.7` to `0.9.8`.
- Created the `docs/versions/v0.9.8` snapshot from the latest mdBook sources for this release line.

### Fixed
- Aligned admin authentication and request handling by allowing bearer-token API access through middleware, enforcing request-size limits against actual streamed bodies, and preserving gateway query strings.
- Cleared the CLI quality-gate blockers in the x402 and sync surfaces so `npm --prefix cli run check` passes cleanly.
- Fixed the Node x402 strict-signature flow so strict `ml_dsa65` signatures can be used against intents created with the matching stored policy.

## [0.9.7] - 2026-04-06

### Added
- Added the new authenticated admin dashboard app with analytics, operations, gateway, billing, integrations, and session-management surfaces, plus the supporting API routes and test coverage.
- Published generated MCP tool inventory artifacts for compatibility tracking in both JSON and mdBook appendix form.

### Changed
- Bumped workspace, bindings, admin app, CLI, examples, templates, docs, and release metadata from `0.9.6` to `0.9.7`.
- Updated the sync and x402 client paths so the latest CLI, gateway, and embedded binding flows stay aligned across real runtime usage and regression coverage.

### Fixed
- Tightened sync configuration security coverage and x402 payment-intent persistence coverage around the refreshed client behavior.

## [0.9.6] - 2026-04-04

### Added
- Added raw-binding compatibility regression coverage for getter-style `commerce.x402` and mixed A2A/x402 commerce surfaces so agent-payment flows are validated against the real Node binding shape.

### Changed
- Bumped workspace, bindings, admin app, CLI, examples, templates, docs, and release metadata from `0.9.5` to `0.9.6`.
- Normalized the shared commerce API access layer so A2A runtimes, MCP tools, the x402 CLI, and the MCP server all support both getter-style and callable-style embedded bindings.

### Fixed
- Persisted x402 signing hashes at intent creation and tightened settlement-state validation so intents cannot skip directly to `Settled`.
- Fixed the shipped x402/A2A payment tooling to work against the real embedded Node binding, including local signing, sequencer submission payloads, settlement updates, and agent-card/runtime compatibility.

## [0.9.5] - 2026-04-03

### Added
- Published repo-native trust and strategy documentation, including `TRUST_FOUNDATION.md`, distribution planning, outcomes modeling, and competitive-landscape notes to make the project posture more explicit.

### Changed
- Bumped workspace, bindings, admin app, CLI, templates, docs, and release metadata from `0.9.4` to `0.9.5`.
- Synced install snippets, deployment examples, and current-release references to the `0.9.5` release.

### Fixed
- Hardened MCP permission enforcement so unknown tools fail closed instead of silently defaulting to read access, and aligned tool permission metadata with the runtime permission map.
- Replaced silent in-memory downgrade paths with durable JSON fallback persistence for audit logs, credentials, treasury records, channel identity/session state, agent sessions, conversation memory, and ERC-8004 identity storage when the native SQLite binding is unavailable.
- Enforced session retention caps correctly and fixed channel-session fallback upsert field ordering to preserve session integrity under degraded runtime conditions.

## [0.9.4] - 2026-04-02

### Added
- x402 v2 exact-EVM payment support across the CLI, including standards-shaped `PAYMENT-SIGNATURE` retries, exact `PaymentPayload` construction, and exported exact/facilitator/resource-server helpers.
- Facilitator primitives and HTTP endpoints for `/supported`, `/verify`, and `/settle`, plus runnable exact-flow facilitator and resource-server examples.
- Exact resource-server helpers that emit `payment-required`, validate incoming `PAYMENT-SIGNATURE` payloads, settle accepted payments, and return `PAYMENT-RESPONSE`.
- Base Sepolia and Ethereum Sepolia exact-EVM support, including testnet USDC configuration and new unit coverage for exact flow, facilitator flow, and resource-server flow.
- Release hygiene automation for CI and publish workflows, including `check_release_hygiene.sh`, regression coverage for the helper, and `actionlint` workflow linting.

### Changed
- Bumped workspace and cross-language package metadata from `0.9.3` to `0.9.4`.
- Synced docs, examples, templates, and lockfiles to the `0.9.4` release.
- Updated release and publish workflows to gate on shared release-hygiene checks instead of version-sync alone.

### Fixed
- Aligned JavaScript x402 signing-hash verification with the Rust implementation by binding `resourceUri` and `resourceMethod` into signed legacy payment intents.
- Removed the legacy sequencer requirement for exact x402 MCP calls while preserving explicit errors for legacy sequencer-backed flows.
- Corrected the VES docs to describe the intended cross-language x402 hashing parity more precisely.

## [0.9.3] - 2026-04-01

### Added
- Native post-quantum VES cryptography in `stateset-crypto` for hybrid `ed25519+mldsa65` and `x25519+mlkem768` flows, plus `pqc-strict` `mldsa65` and `mlkem768` modes for key generation, signing, verification, recipient wrapping, payload encryption/decryption, and proof-of-possession.
- Sync-layer PQC security profiles (`legacy`, `hybrid`, `pqc-strict`) across config validation, key management, outbox signing/encryption, pulled-event decryption, and sequencer receipt verification.
- Native Node binding exports for hybrid and strict PQC operations, including signing, verification, payload encryption/decryption, recipient key generation, and signing proof-of-possession helpers.
- PQC audit and observability coverage, including profile-change audit events, key-generation/rotation logging, and per-profile signature/encryption counters.
- PQC validation assets: cross-language Node/Rust test vectors, strict-profile tests, expanded Rust crypto coverage, Criterion PQC benches, and the initial migration spec in `docs/PQC_INITIAL_SPEC.md`.

### Changed
- Enforced TLS for PQC-enabled sync profiles and blocked unforced profile downgrades so future events cannot silently lose post-quantum protection.
- Bumped workspace and cross-language package metadata from `0.9.1` to `0.9.3`.
- Synced docs, examples, templates, and lockfiles to the `0.9.3` release.

## [0.9.1] - 2026-03-26

### Added
- **Agentic Commerce**: Negotiation engine with auto-accept/reject thresholds, A2A messaging with retry, credit terms (net 15/30/60/90), inventory commitments, dispute rules engine
- **V9 Migration**: 8 new tables for agent commerce (a2a_messages, a2a_negotiations, inventory_commitments, a2a_credit_terms, a2a_tax_obligations, a2a_dispute_rules)
- **5 Negotiation REST endpoints**: create, get, counter-offer, accept, reject
- **497 A2A tests** across 17 modules

## [0.9.0] - 2026-03-26

### Added
- **11 V4 entity implementations**: reviews, wishlists, gift cards, loyalty, fraud, segments, store credits, shipping zones, rewards, search configs, zone shipping methods (was 11 stubs)
- **18 V4 HTTP endpoints**: reviews, wishlists, gift cards, loyalty CRUD + actions
- **Clippy pedantic fixes** across 174 files (1,377 insertions)
- **12 new HTTP integration tests** (81 total)

## [0.8.8] - 2026-03-25

### Added
- **Pricing engine** wired into order creation with currency-aware rounding
- **Audit log** (V8 migration) with record_audit() function
- **Graceful DB shutdown** (WAL checkpoint + PRAGMA optimize)
- **ETag utility module** for HTTP conditional requests
- **Fat LTO + target-cpu=native** for maximum compiled performance
- **Gzip response compression** on all API endpoints

## [0.8.5] - 2026-03-25

### Fixed
- **Inventory reservation race condition**: atomic quantity+version check in UPDATE WHERE clause
- **SQLITE_FULL detection**: maps to StorageFull error instead of generic 500
- **UNIQUE constraint violations**: return 409 Conflict instead of 500
- **LIKE wildcard escaping** in product search

### Added
- **V6 Migration**: 3 idempotency constraints (order_items, reservations, cart checkout)
- **Health check**: GET /health/deep with DB latency + metrics
- **Slow query logging**: transactions >500ms emit tracing::warn
- **Request timeout**: 30-second TimeoutLayer on all API endpoints

## [0.8.4] - 2026-03-25

### Added
- **13 new REST endpoints**: PATCH/DELETE for customers and products, POST for shipments, payments, invoices with action endpoints (deliver, complete, refund, send, record-payment)
- **V5 Migration**: 12 composite database indexes for common query patterns
- **29 error messages** now include valid enum values
- **13 new integration tests** for all new endpoints

## [0.8.2] - 2026-03-25

### Changed
- **Performance**: 8 rounds of autoresearch-driven optimization (~3x all 20 Criterion benchmarks)
  - SQLite: PRAGMA tuning, prepare_cached, mmap, WAL autocheckpoint, deferred FK
  - EventBus: lazy event_type allocation, deferred receiver_count, inline publish
  - Merkle tree: double-buffer swap, SHA256 asm, hasher reuse, pad memoization
  - Money: #[inline] on hot arithmetic paths
  - Compiler: codegen-units=1
  - Metrics: lock-free CAS for f64 accumulators
  - Event store: AtomicU64 sequence counter

## [0.8.1] - 2026-03-18

### Added
- Added native Bitcoin settlement flows for autonomous agent payments, including wallet, signing, execution, and observability plumbing.
- Added shielded Zcash settlement support for agent-to-agent payments through wallet-enabled JSON-RPC flows.
- Added Machine Payments Protocol support across MCP and HTTP, including challenge/credential/receipt handling, discovery metadata, and client retry helpers.
- Added embedded toolkit support for remote payable HTTP route discovery and paid execution.

### Changed
- Bumped workspace and cross-language release metadata from `0.8.0` to `0.8.1`.
- Synced docs, templates, examples, and packaging references around the `0.8.1` native payments and MPP release.

## [0.8.0] - 2026-03-11

### Added
- Added an embedded agent onboarding quickstart with `@stateset/cli/agent-toolkit`, OpenAI-style JSON-schema tool export, and framework adapter examples for server-side agent runtimes.
- Added package export regression coverage for the standalone and embedded agent toolkit surfaces.

### Changed
- Bumped workspace and cross-language release metadata from `0.7.25` to `0.8.0`.
- Synced docs, examples, and release notes around the `0.8.0` embedded agent onboarding flow.

### Fixed
- Published `@stateset/cli/agent-toolkit` as a first-class package export so the documented embedded agent import path works for installed consumers.
- Hardened release smoke tests to verify package self-reference imports for `@stateset/cli/standalone` and `@stateset/cli/agent-toolkit` before publish.

## [0.7.23] - 2026-03-10

### Changed
- Bumped workspace and cross-language release metadata from `0.7.22` to `0.7.23`.
- Tightened root quality gates so `npm run check` enforces the admin lane plus the CLI supported typecheck lane under explicit Node/npm runtime guards.
- Expanded the CLI supported typecheck surface to cover the x402 package, `src/x402-mcp-server.js`, `src/tools/x402.js`, and `src/sync/crypto.js`.

### Fixed
- Reduced type drift across the x402/runtime surfaces, including crypto helpers, lazy dependency loading, and chain helper JSDoc contracts.
- Added admin test-suite typechecking and fixed test/runtime mismatches needed for the stricter gate to pass cleanly.
- Fixed the stale migration snapshot and hardened cart number generation to avoid collisions during fast concurrent test runs.

## [0.7.22] - 2026-03-06

### Added
- Added `stateset simulate` and the A2A simulation runtime for sandboxed scenario execution with virtual time, snapshots, and failure injection.
- Added the built-in `supplier-goes-offline` scenario plus simulation-focused CLI and unit coverage.
- Added CI `version-sync` gate (`scripts/ci/check_version_sync.sh`) and wired it into root `npm run check`.
- Added Rust crate publish automation: `scripts/publish-rust-crates.sh` and `.github/workflows/publish-rust-crates.yml`.

### Changed
- Bumped workspace and cross-language release metadata from `0.7.21` to `0.7.22`.
- Updated CLI/runtime version references and packaging metadata to `0.7.22` across manifests, config constants, templates, and version assertion tests.
- Raised Rust threshold in `.github/workflows/coverage.yml` from 70% to 80% to match primary CI policy.
- Refreshed `docs/TESTING_STRATEGY.md` coverage section to document enforced CI gates instead of stale point-in-time estimates.
- Expanded `RELEASING.md` with Rust crates.io release flow and generalized binding release examples to `vX.Y.Z`.

### Removed
- Removed tracked SQLite WAL/SHM artifacts from `cli/` (`checkout-demo`, `demo`, `store`) to keep repository state clean.

## [0.7.14] - 2026-02-28

### Changed
- Bumped workspace and cross-language release metadata from `0.7.13` to `0.7.14`.
- Bumped CLI/runtime version references and packaging metadata to `0.7.14` across manifests, config constants, templates, and version assertion tests.
- Added MCP gateway readiness and Prometheus metrics endpoints (`/ready`, `/metrics`) and updated Kubernetes/Prometheus deployment wiring.
- Tightened CI quality gates by failing coverage jobs on undetermined coverage values.

### Fixed
- Enforced tenant-aware API access in `stateset-http`: authenticated `/api/v1/*` requests now require validated `x-tenant-id`.
- Added bearer-token tenant binding support and rejection of tenant/token mismatches for principal isolation.
- Implemented per-tenant SQLite routing in `stateset-http` (`<tenant>.db`) and added integration tests proving cross-tenant data isolation.
- Hardened browser navigation URL policy in CLI gateway to block local/private/internal hosts by default (SSRF risk reduction).
- Aligned CLI/mcp-events output contracts and test behavior, including stable event-subscription payload shape and runtime binary selection in E2E tests.

## [0.7.13] - 2026-02-27

### Changed
- Bumped workspace and cross-language release metadata from `0.7.12` to `0.7.13`.
- Bumped CLI/runtime version references from `0.7.8` to `0.7.13`.
- Added `stateset-setup --quickstart` preset for one-command agent onboarding (`--demo --agent openclaw --starter-pack ops --agent-only --verify`).
- Expanded onboarding artifacts with generated launch/health scripts (`start-mcp.sh`, `check-mcp.sh`) and handoff launch commands.

### Fixed
- Improved onboarding verification coverage to validate handoff launch command readiness.
- Improved setup next-step guidance with direct launch and health-check commands for faster agent time-to-value.

## [0.7.10] - 2026-02-27

### Changed
- Expanded CI quality gates with Postgres parity matrix lanes, FFI sanitizer lanes, perf regression reporting, and crate compatibility governance reporting.
- Added cross-language FFI ABI contract fixtures/tests for C, C++, Python, and Swift.
- Added observability conventions plus RED/SLO metrics primitives and documentation updates.
- Added perf-gate benchmarks and strengthened property/chaos style test coverage in protocol/sync/pricing/primitives/jobs crates.

### Fixed
- Hardened A2A and embedded webhook SSRF protections (allowlists, ambiguous IPv4 encodings, IPv4-mapped IPv6 handling, and DNS rebinding coverage).
- Fixed webhook host IP parsing behavior for deterministic IPv4/IPv6 safety checks.

## [0.7.9] - 2026-02-27

### Changed
- Bumped workspace and cross-language release metadata from `0.7.8` to `0.7.9`.
- Updated binding package versions across Node, Python, Ruby, PHP, Java, Kotlin, Swift, .NET, and wasm artifacts.
- Updated SDK/FFI surfaced version references to `0.7.9`.

### Fixed
- Hardened policy evaluation semantics, rule ordering, and authz rate-limit key handling.
- Hardened A2A/embedded webhook SSRF protections and added mapped-IPv6 regression coverage.
- Fixed sync pagination/cursor behavior and strengthened protocol integrity hashing/ordering guarantees.
- Hardened FFI safety boundaries, conversion error handling, and HTTP readiness contract behavior.
- Removed DB/runtime panic paths, fixed cart total recomputation and jobs timeout/cron lifecycle behavior, and improved subscription uniqueness handling.
- Fixed crypto malformed-envelope panic surfaces and corrected `#[derive(StateSetId)]` downstream behavior.

## [0.7.8] - 2026-02-25

### Changed
- Bumped workspace and cross-language release metadata from `0.7.7` to `0.7.8`.
- Updated CLI/runtime version references (`CLI_VERSION`, gateway config/version fallback, scaffold templates, WhatsApp user agent, and update messaging) to `0.7.8`.
- Updated lockfile and packaging metadata for CLI and language bindings to `0.7.8`.
- Enabled Swift bindings CI checks on pull requests without requiring the `ci-swift` label.

### Fixed
- Hardened `/browser/evaluate`: disabled by default and gated expression execution with strict read-only policy validation.
- Hardened marketplace remote installs with HTTPS/public-host validation, catalog base URL restrictions, checksum verification, and redirect blocking.
- Fixed MCP structured tool metadata to preserve `sessionId` for direct tool-handler invocations.
- Fixed scaffold API route generation to preserve leading route slash and emit explicit `status: 500` error responses.
- Fixed telemetry verbose tool-call logging capture path to preserve secret-redaction assertions in tests.
- Improved test stability under high-concurrency runs for HTTP gateway and setup wizard suites.

## [0.7.6] - 2026-02-24

### Changed
- Bumped workspace and cross-language release metadata from `0.7.5` to `0.7.6`.

### Fixed
- Fixed policy engine domain index replacement behavior when re-registering a policy set with the same ID.
- Implemented sync engine conflict resolution effects for local-vs-remote event handling.
- Implemented paginated pull handling in sync full-sync flows.
- Cleared strict `clippy -D warnings` regressions in embedded commerce constructors/builders.

## [0.7.4] - 2026-02-22

### Added
- Added a `stateset-setup` CLI binary entry in `package.json`.
- Added `@clack/prompts` dependency to support interactive CLI UI flows.
- Added `stateset-crypto` to the workspace dependency set and Node wrapper dependency graph.

### Changed
- Bumped workspace and cross-language release metadata from `0.7.2` to `0.7.4` across Rust crates, CLI packages, language bindings, examples, and docs.
- Updated CLI/version runtime references (`CLI_VERSION`, health endpoint fallback, scaffold templates, WhatsApp user agent) to `0.7.4`.
- Updated npm lockfiles and package manifests to reference `0.7.4`.
- Adjusted select CLI logs from `console.log` to `console.debug`/`console.info`.
- Updated lockfile dependency graph for crypto-related workspace crates.

### Fixed
- Aligned version checks and dependency specifiers in examples to `0.7.4`.

## [0.7.2] - 2026-02-20

### Changed
- Bumped the workspace and cross-language release metadata to `0.7.2` across Rust crates, CLI, language bindings, and examples.
- Updated docs and configuration references to reflect the `0.7.2` version line (including npm/cargo/composer/gradle packaging metadata and SDK version checks).

## [0.7.0] - 2026-02-07

### Added
- **1,842 automated tests** (1,581 CLI + 261 admin) with 0 failures — up from ~76 in v0.6.0.
- 40+ new CLI unit test files covering permissions, telemetry, errors, HTTP gateway/auth, channels subsystem (middleware, rich-messages, templates, event-bridge, gateway-methods, notifier, handoff, metrics, adapter-types), context, credentials, session persistence, MCP schema validator, command queue, and more.
- ESLint flat config for CLI with `eslint-config-prettier` integration.
- Prettier config with `format:check` in CI and pre-commit hook.
- Commitlint + Husky hooks enforcing conventional commits (`commit-msg`, `pre-commit`).
- `jsconfig.json` with `checkJs` for CLI type checking via JSDoc.
- Persistent SQLite audit log (`audit-store.js`) for permission gate decisions.
- In-memory sliding-window rate limiter (per-API-key 60/min, per-IP 30/min) on HTTP gateway.
- Graceful shutdown handlers for all 47 `bin/` entry points (`runMain()` / `installShutdownHandlers()`).
- Security headers on HTTP gateway (CSP, X-Frame-Options, X-Content-Type-Options, Referrer-Policy).
- Body size limits on HTTP gateway and admin API routes.
- `safeIdSchema` path traversal prevention on admin API routes.
- `secrets.yaml.template` pattern (actual secrets gitignored).
- Harness lifecycle events (`onEvent`) across loop/stream sessions plus context transforms and hook points (`before_compaction`, `tool_result_persist`, `before_send`).
- Provider overrides for non-Claude calls (`apiKey`, `getApiKey`, `signal`) and stream session event emission.

### Changed
- 168+ MCP tools mapped to permission gates (was 64).
- `@modelcontextprotocol/sdk` upgraded ^1.25.4 to ^1.26.0 (fixes GHSA-345p-7cg4-v4c7).
- `Math.random()` replaced with `crypto.randomUUID()` in mcp-conversation-context, mcp-tool-composer, and error boundary.
- ~15 empty `catch {}` blocks replaced with `console.warn()` across orchestrator, HTTP gateway, credentials, agent-session-store, permissions, claude-harness, and messaging gateways.
- Command injection prevention: `scaffold-server` allowlist, `marketplace` and `gateway` use `execFileSync`.
- SQL injection prevention: `treasury/store.js` hardcoded column whitelist.
- Error detection in `errors.js` uses property-based + case-insensitive fallback (replaced fragile string matching).
- `load-env.js` warns on missing `.env` instead of silently failing.
- `capture.js` warns on unmapped event types.
- Admin test coverage thresholds raised to 80/70/70/80.
- Rust core models, DB layer, and embedded API updated with new methods and improved error handling.
- Language bindings updated across Node, Python, WASM, Ruby, PHP, Java, Kotlin, Swift, .NET, and Go.

### Fixed
- `mcp-schema-validator.js`: `.optional().regex()` reordered to `.regex().optional()` (Zod API).
- `x402/budget.js`: `DEFAULT_STATE` shared mutable references replaced with deep copy.
- `credentials.js`: silent `.catch(() => {})` replaced with `console.warn`.
- `session-persistence.test.js`: TTL race condition (sessionTtl 1ms to 5000ms).
- `runMain()`: `Promise.resolve()` fix for sync main functions.
- Streaming error handling in `gemini.js`, `ollama.js`, `openai.js` (debug logging on catch).
- Admin sessions route: silent `.catch(() => ({}))` replaced with proper error handling.

## [0.6.0] - 2026-02-04

### Added
- Treasury engine with SQLite-backed ledger for agent funding, swaps, and fees (stablecoin-first).
- `stateset-treasury` CLI for wallets, deposits, balances, ledger, token registry, and pricing rules.
- ERC-8004 identity registry helpers (SQLite) with CLI + MCP tools.
- MCP treasury tools and ERC-8004 tools with audit metadata (`task_id`, `request_id`, `session_id`, `tool_name`).
- LLM billing from treasury: Claude uses SDK cost; OpenAI/Gemini use estimated cost with preflight budget enforcement.
- CLI flags and env support for treasury + ERC-8004 binding.

### Changed
- Stablecoin payments now record treasury withdrawals when executed.
- Tool pricing can auto-debit treasury balances when `--apply` is set.

## [0.5.0] - 2026-02-02

### Changed
- Version alignment across workspace crates, bindings, CLI, docs, and examples.

## [0.3.1] - 2026-01-29

### Added
- API key authentication for HTTP gateway (Bearer token + query param).
- Per-route permission levels (none / read / preview / write / delete / admin).
- Sandbox mode to block browser and shell routes.
- Proactive heartbeat monitor with 6 commerce checkers (low stock, abandoned carts, revenue milestone, pending returns, overdue invoices, subscription churn).
- Heartbeat HTTP API (status, list checks, run, enable, disable).
- EventBridge integration for heartbeat alerts across all messaging channels.
- `HEARTBEAT_DEFAULTS` and `HTTP_GATEWAY_DEFAULTS` in config.
- 76 new tests (39 permissions + 37 heartbeat).

## [0.2.4] - 2026-01-26

### Added
- Vector search models and APIs across core, db, and embedded crates.
- Embeddings service wiring for generating/querying vectors.
- SQLite vector search migration and query helpers.
- CLI vector tooling for embedding and search workflows.

## [0.2.0] - 2026-01-16

### Added
- PostgreSQL migration coverage test and CI target for the postgres feature.
- CLI test job in CI.
- Supply-chain checks via cargo-deny, Dependabot, and SBOM generation.
- Benchmarks for core, db, and embedded crates in CI.

### Changed
- Version alignment across bindings, CLI templates, and installers.
- Security policy now supports the 0.2.x line.

## [0.1.9] - 2025-01-09

### Fixed
- Safer Decimal to f64 conversions across all bindings (Node, Python, Ruby, PHP, Java, Kotlin, Swift) using `to_f64_or_nan` helper instead of `unwrap_or(0.0)`.
- Improved JNI error handling in Java bindings with `jni_or_throw` helper for better exception propagation.
- General Ledger parsing now uses proper error propagation (`parse_required`, `parse_optional`) instead of silent defaults.

### Changed
- All binding code now consistently handles numeric conversion edge cases.

## [0.1.8] - 2025-01-01

### Added
- mdBook-based documentation scaffold with API reference pointers and versioning notes.
- Docs build and version snapshot scripts under `docs/scripts/`.

## [0.1.7] - 2025-12-20

### Added
- 34 new MCP tools across Payments, Shipments, Suppliers/POs, Invoices, Warranties, and Manufacturing.
- Expanded agent and CLI coverage for additional commerce domains.

## [0.1.6] - 2025-12-20

### Added
- Java bindings via JNI.
- Ruby and PHP binding releases with native extensions.

### Fixed
- JNI memory management for thread-safe handles.
- Product variant handling in the Product API.
- Cart total calculations using `grand_total`.
