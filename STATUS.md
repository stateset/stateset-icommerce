# StateSet iCommerce — Engineering Status

Living document tracking the Q2 2026 quality and feature elevation campaign
that closed the gaps surfaced by the comprehensive codebase grading review.
Originated from the review's "let's put together a plan and execute" ask.

## At a glance

The plan is **substantially complete**: 11 of 13 phases closed
(Phase 1.1, 1.2, 1.3, 1.4, 2, 4.1, 4.3-frontend, 4.4, 5, 6.3, 6.4),
3 in flight with diminishing-returns positioning (3.1, 4.2 at 5/6,
6.2), and 1 strategic-stretch phase intentionally untouched (Phase
7: PQC hard finality, SOC 2 Type I, formal verification).

| Area | Delivered |
| --- | --- |
| **Plan phases closed** | **11 / 13** |
| Plan phases in flight | **3** (3.1 diminishing returns; 4.2 5/6 audit-flagged closed; 6.2 mid-README dedup ~done) |
| Plan phases untouched | **1** (Phase 7 — strategic stretch only) |
| New unit tests (`stateset-db`) | **+253** across 21 SQLite repository files |
| New property tests (`stateset-policy` + `stateset-sync`) | **+30 properties × ~256 cases each** |
| New CLI MCP-extraction tests | **+246** across 13 extracted modules |
| New admin component + helper tests | **+171** (62 UI primitives + 80 Phase 4.2 helpers + 23 Phase 4.1 components + 6 Phase 4.4 build-info) |
| Production bugs found and fixed | **9 critical** |
| `cli/src/mcp-server.js` reduction | **5,309 → 4,065 lines (-23.4%)** |
| `README.md` reduction | **1,770 → 1,439 lines (-18.7%)** + scannable TOC + capability matrix |
| Cross-binding parity tests | **44 tests across 11 implementations** (Rust + Node + Python + Go + WASM + Java + Kotlin + .NET + Swift + Ruby + PHP) |
| Postgres parity tests | **13 dedicated test files** in 3-way CI matrix on `postgres:16` |
| Fuzz targets (nightly) | **6** (3 crypto, 3 protocol) |
| Supply-chain audited deps | **138 fully + 6 partially / 408 exempted** at bootstrap |
| HTTP API endpoints | **30+ REST + SSE event stream + new `/version` build-info endpoint** |
| Admin pages with coverage gate | **OrgSwitcher, RmaInbox, BulkOrders, AuditLog, ExportHub, BuildInfo** |

## What's next

With 11 of 13 audit phases closed, the remaining work breaks into
three clean buckets:

1. **Diminishing-returns extraction (Phase 3.1)** — `cli/src/mcp-server.js`
   has dropped 23.4% across 13 modules. The remaining ~4,000 lines
   are mostly closures over runtime state (commerce instance,
   telemetry, hookRunner, policyEngine, agenticReplayRingBuffer).
   Further extraction past this point requires careful interface
   design; the orchestrator is now reasonably tight.

2. **Backend-blocked frontend items** — Phase 4.2's webhook UI and
   Phase 4.3's `/api/v1/organizations` endpoint both need engine-side
   API surfaces before the admin pages can graduate from their
   dev-mode placeholders. Tracked but parallel to the platform plan.

3. **Strategic stretch (Phase 7)** — PQC hard finality, SOC 2 Type I,
   formal verification of policy DSL + sync convergence. Each is a
   multi-month workstream with external dependencies (auditors,
   formal-methods tooling, NIST guidance updates). Intentionally
   untouched until pre-requisites are scoped.

Day-to-day quality maintenance now goes through the existing CI
gates: 11 phase-completion test suites, the cross-binding parity
matrix (44 tests across 11 implementations), the Postgres parity
matrix (13 tests on `postgres:16`), and the regular workspace
test suite (~16K tests).

## Production bugs surfaced and fixed

The grading review predicted that adding unit-test coverage to load-bearing
files would surface latent bugs. Nine were found and fixed during Phase 1.1:

1. **22 SQLite triggers writing non-RFC3339 timestamps** — every `UPDATE`
   on warehouses, receipts, picks/packs/ships, AP/AR, GL, credit, or
   backorders was silently corrupting the `updated_at` column so
   subsequent reads crashed in the chrono parser. Migration 036 fixes
   all 22 triggers.
2. **AR `get_invoices_due_for_dunning` referenced wrong column names**
   (`issue_date, tax, shipping, discount` vs actual
   `invoice_date, tax_amount, shipping_amount, discount_amount`).
3. **AR `get_average_days_to_pay`** — same `issue_date` typo.
4. **`generate_lot_number` race** — second-granularity timestamps
   collided on `UNIQUE` constraint under concurrent / batch creation.
5. **`list_suppliers` ignored `name`, `country`, and `offset`** filter
   fields silently — only `active_only` and `limit` actually worked.
6. **Cost accounting `get_inventory_valuation` + `get_sku_cost_summary`**
   queried `inventory_items.quantity_on_hand` (column lives on
   `inventory_balances`).
7. **`generate_warranty_number` + `generate_claim_number` race** —
   millisecond-precision still collided in batch flows.
8. **`generate_inspection_number` + `generate_ncr_number` race** —
   second-precision collided.
9. **Analytics `get_low_stock_items`** queried
   `inventory_balances.on_hand`/`allocated` (real columns are
   `quantity_on_hand`/`quantity_allocated`); `reorder_point` lives on
   balances, not items.

Pattern: every untested file with > 1k LOC contained at least one
schema-drift, race-condition, or filter-completeness bug.

## What landed, by phase

**Quick scoreboard** — the per-phase deep dives below carry the detail.

| Phase | Status | One-line summary |
| --- | --- | --- |
| 1.1 SQLite repo tests | ✓ closed | 21/21 untested repos covered; 9 production bugs surfaced and fixed |
| 1.2 Postgres parity | ✓ closed | 13 dedicated test files in 3-way `postgres:16` CI matrix |
| 1.3 Property tests | ✓ closed | 30 properties × ~256 cases across `stateset-policy` + `stateset-sync` |
| 1.4 Sync outbox/verifyInclusion | ✓ closed | Two known sync bugs documented + regression-tested |
| 2 Security & supply chain | ✓ closed | gitleaks, CodeQL Rust, fuzz-nightly, supply-chain audit, signed releases workflow |
| 3.1 Orchestrator decomposition | ⏳ in flight | 13 modules extracted, mcp-server.js -23.4% (5,309 → 4,065 lines), 246 tests |
| 4.1 Admin component testing | ✓ closed | 6/6 UI primitives + 4 client components under coverage gate |
| 4.2 Operational workflows | ⏳ 5/6 | Audit log, RMA inbox, bulk orders, CSV export, multi-org switcher closed; webhook UI blocked on backend |
| 4.3 Multi-org switcher | ✓ closed (frontend) | TopBar + listOrganizations + layout integration; engine endpoint pending |
| 4.4 Build & Release verification | ✓ closed | `/version` endpoint + `/build-info` admin page + operator build recipe |
| 5 Bindings parity | ✓ closed | 10/10 bindings wired; 44 cross-binding tests across 11 implementations |
| 6.2 README polish | ⏳ in flight | -18.7% (1,770 → 1,439 lines), TOC + capability matrix |
| 6.3 Stale doc tool counts | ✓ closed | 700+ MCP tools across 63 domain modules (was 365+ in 8 places) |
| 6.4 Public security overview | ✓ closed | `docs/src/security/overview.md` + mdBook + README link |
| 7 Strategic stretch | untouched | PQC hard finality, SOC 2 Type I, formal verification — multi-month workstreams |

### ✓ Phase 1.1 — SQLite repo unit tests *(closed)*

21/21 large untested SQLite repository files now have inline `#[cfg(test)]`
modules: `inventory`, `carts`, `general_ledger`, `accounts_receivable`,
`serials`, `lots`, `purchase_orders`, `cost_accounting`, `work_orders`,
`tax`, `warehouse`, `promotions`, `warranties`, `shipments`,
`accounts_payable`, `fulfillment`, `invoices`, `quality`, `products`,
`credit`, `analytics`. Test count: 165 → 418 (+253, +153%).

### ✓ Phase 1.3 — Property tests *(closed)*

- `crates/stateset-policy/tests/proptest_operator.rs` — 22 properties
  asserting truth-table invariants for all 20 `Operator::evaluate` arms
  (Eq reflexivity/symmetry, In/NotIn duality, Lt/Gt anti-symmetry,
  Between endpoints, IsEmpty/IsNotEmpty duality, Contains/StartsWith
  reflexivity, unary-ignores-compare invariant, DivisibleBy by self/1,
  cross-type Eq strictness).
- `crates/stateset-sync/tests/proptest_conflict.rs` — 8 properties on
  `ConflictResolver` (RemoteWins/LocalWins totality, LastWriterWins
  timestamp ordering + tie-to-local + swap anti-symmetry,
  `resolve_batch` matches per-pair, strategy round-trip,
  SyncEvent hash determinism).

~7,700 random cases per CI run.

### ✓ Phase 1.4 — Sync outbox / verifyInclusion bugs *(closed; verified)*

Both flagged JS sync bugs were already fixed in current code:
`outbox.append() aadParams` correctly include all 4 fields,
`client.verifyInclusion()` calls `computeLeafHash` with the correct shape.

### ✓ Phase 2 — Security & supply chain *(closed)*

- `.github/workflows/gitleaks.yml` — full-history secret scan on push + PR.
- CodeQL extended to **Rust + JavaScript + Actions** with manual-mode
  Rust build via Swatinem/rust-cache.
- `.husky/pre-commit` runs `cargo fmt --check` + `cargo clippy -D warnings`
  when `*.rs` / `*.toml` is staged. Skippable via `SKIP_RUST=1`; no-ops
  when cargo isn't on PATH.
- `crates/stateset-crypto/fuzz/` —
  3 cargo-fuzz targets (canonicalize_json, compute_payload_plain_hash,
  compute_merkle_root) running 90 s nightly each via
  `.github/workflows/fuzz-nightly.yml`.
- `supply-chain/{config,audits,imports.lock}.toml` — cargo-vet bootstrap
  with 6 trusted import feeds (Mozilla, Google, Embark Studios, Bytecode
  Alliance, Zcash, ISRG). Initial vet: 138 fully audited, 6 partially,
  408 exempted, 0 unvetted.
- `.github/workflows/release-sign.yml` — sigstore cosign keyless OIDC
  signing on every `v*` tag. Builds source tarball + CycloneDX SBOM,
  computes SHA256SUMS, signs with the workflow's ephemeral OIDC token
  (recorded in Rekor), uploads to GitHub Release.
- `SECURITY.md` extended with supply-chain section + signed-release
  verification recipe.

### ⏳ Phase 3.1 — Orchestrator decomposition *(in flight)*

Extracted 13 focused modules from `cli/src/mcp-server.js` (5,309 → 4,065
lines, **-23.4%**):

| Module | Lines | Tests | What it owns |
| --- | --- | --- | --- |
| `cli/src/mcp/replay-sanitizer.js` | 150 | 26 | Replay-log sanitization (redaction + size caps + cycle detection) |
| `cli/src/mcp/cost-budget.js` | 165 | 26 | Cost summary aggregation + per-bucket budget resolution |
| `cli/src/mcp/plan-resolver.js` | 162 | 25 | Agentic-plan template substitution + path resolution |
| `cli/src/mcp/agentic-runtime-tools.js` | 493 | 10 | 15 agentic-runtime tool descriptors (data + handlers) |
| `cli/src/mcp/compensation.js` | 175 | 21 | Saga compensation hints + ID-extraction helpers + buildCompensationParams |
| `cli/src/mcp/policy-domain.js` | 142 | 20 | Static + dynamic policy-domain inference + token-to-domain table |
| `cli/src/mcp/policy-helpers.js` | 81 | 15 | Tool-name normalization + policy transform applicator |
| `cli/src/mcp/auto-index.js` | 50 | 9 | Background vector auto-indexing fan-out for new entities |
| `cli/src/mcp/commerce-adapter.js` | 156 | 20 | Callable Proxy accessor + Commerce prototype-chain hoist + API decorator |
| `cli/src/mcp/plan-step-routing.js` | 80 | 10 | Per-step agent-routing decision (router function injected) |
| `cli/src/mcp/audit-signing.js` | 70 | 14 | HMAC-SHA256 signed (or deterministic-unsigned) audit envelopes |
| `cli/src/mcp/audit-envelope.js` | 175 | 24 | replayEventHash + policy-action/explanation normalize + rollback contract + approval-stage extraction |
| `cli/src/mcp/mutation-manifest.js` | 130 | 26 | Idempotency-key extraction + deterministic mutation manifest builder |

246 unit tests across 57 suites covering all 13 modules.

### ✓ Phase 4.1 — Admin component testing *(complete)*

- 6/6 design-system primitives now under coverage gate
  (Button, Badge, Card, Progress, LoadingSkeleton, ErrorBoundary).
- Component tests for the new admin pages: `<OrgSwitcher />`
  (renders-nothing-on-single-option, dropdown rendering, server-action
  dispatch, router refresh), `<RmaInboxClient />` (status-aware
  action gating per `requested`/`approved`/`received`/`refunded`,
  filter toggle, bulk-select interaction), `<AuditLogClient />`
  (10 tests covering connection-state transitions, paused/resume,
  clear, filter narrowing, export-button gating, EventSource cleanup
  on unmount), and `<ExportHubClient />` (4 tests covering the three
  entity cards, column-count badges, accessible labels).
- Vitest config gates the new files under existing 80%/70% thresholds;
  `audit-log-client.tsx` and `export-hub-client.tsx` are now in the
  coverage `include` list so regressions in rendered behavior fail the
  build.

### ⏳ Phase 4.2 — Operational workflows *(5/6 audit-flagged closed)*

| Audit-flagged gap | Status | Where |
| --- | --- | --- |
| Audit log viewer | ✓ | `/audit` page subscribing to engine SSE feed |
| RMA inbox | ✓ | `/returns/inbox` with state-aware actions + bulk approve |
| Bulk orders | ✓ | `/orders/bulk` with status filter + cross-status enable rules |
| Reporting / CSV export | ✓ | `lib/csv/csv.ts` + `lib/csv/specs.ts` + `<CsvExportButton>` + `/export` hub |
| Multi-org switcher | ✓ | Cookie + server actions + `<OrgSwitcher />` + `with-error-handler` integration |
| Webhook configuration UI | ☐ | Needs new CLI gateway endpoints (out of scope per firing) |

### ✓ Phase 4.4 — Build & Release verification *(complete)*

Operators ask "is the binary I'm running signed, and where did it come
from?" — a question that previously had no in-app answer. This phase
ships:

- **Backend** *(complete)* — `GET /version` HTTP endpoint at
  `crates/stateset-http/src/routes/health.rs` returning a
  `VersionResponse` DTO with package version, git commit (if injected
  via `GITHUB_SHA`), git ref, release tag (`STATESET_RELEASE_TAG`),
  build timestamp, and a **signed boolean** that defaults to false
  (so unsigned local builds explicitly say so). 3 unit tests + 1
  OpenAPI smoke assertion. Production-zero-cost: all metadata is
  `option_env!`-injected at compile time.
- **Frontend** *(complete)* — `/admin/build-info` page rendering a
  trust badge (green=Signed release, amber=Unsigned build), version,
  release tag (linked to GitHub release), commit SHA (linked to
  GitHub commit, shortened to 12 chars in display, full SHA in href),
  git ref, and built_at timestamp. Surfaces "Engine unreachable"
  with a clear message + `STATESET_API_URL` hint when fetch fails.
  Includes a "How signing works" educational card explaining the
  sigstore + OIDC keyless model. 6 component tests via
  `<BuildInfoView />` (a pure renderer split from the async page so
  tests don't have to mock fetch). Sidebar nav entry under
  ShieldCheckIcon.
- **Operator build recipe** *(complete)* — investigation revealed
  there is no `stateset-http` server binary in the workspace; the
  crate ships as a library and operators compose it into their own
  server (typically a thin `main.rs` that wires routes + state +
  auth). The release pipeline thus has nothing to inject env vars
  *into*. The proper close-out is **documentation**: a recipe
  operators copy-paste into their CI to bake verifiable provenance
  into their own server binaries. Shipped as
  [`docs/src/advanced/build-info-recipe.md`](./docs/src/advanced/build-info-recipe.md)
  with a complete env-var contract table, a GitHub Actions example,
  a verification recipe (curl + admin UI cross-check), and a
  reference back to the Rust source. Wired into the mdBook
  `SUMMARY.md` after the Deployment guide.

**Phase 4.4 complete.** All three sub-deliverables landed:
backend `/version` endpoint (firing #55), frontend `/build-info`
admin page (firing #56), operator build recipe (firing #60). The
default-false `signed` flag on local builds correctly distinguishes
unsigned dev binaries from real release artifacts; the recipe
shows operators exactly which env vars flip it to `true`.

### ✓ Phase 6.3 — Stale doc tool counts *(closed)*

Replaced all 8 occurrences of `365+` in `docs/whitepaper.md` with `700+`.
Actual count: **717 tool entries across 63 domain modules**.

### ✓ Phase 6.4 — Public security overview *(closed)*

`docs/src/security/overview.md` — new landing doc with at-a-glance
defense-layer table (memory safety, panic hygiene, lint posture, supply
chain, static analysis, signed releases, etc.) plus an honest "known
gaps" section calling out PQC hard finality, SOC 2, third-party audit,
and formal verification as planned-not-shipped. Wired into mdBook
SUMMARY and linked from the README front matter.

### ⏳ Phase 6.2 — README front polish *(in flight)*

"Why iCommerce" section landed near the top of the README, surfacing
the agentic stack (A2A, x402, autonomous engine, policy DSL, MCP,
VES v1.0) with deep links. Seven trimming rounds have brought the
README from 1,770 → **1,439 lines (-18.7%)**:

- **Top-of-page Table of Contents** added under the navigation pills
  (collapsible `<details>` block, 16 deep-linked entries) so a long
  README is navigable in one scroll. Anchor links validated.
- **"The Shift: From eCommerce to iCommerce"** section deleted — its
  content duplicated "Why iCommerce" without adding new claims.
- **Installation** section collapsed from 11 per-language install
  snippets (~120 lines) to a 28-line lean replacement that points
  readers at the existing Quick Start (working code) + Language
  Bindings table (install commands), and only retains the genuine
  platform-specific gotchas (Java Maven XML alternative, PHP
  `php.ini` extension config, Swift CocoaPods alternative, CLI
  link procedure).
- **Quick Start "Other bindings"** group: collapsed Ruby, PHP, Java,
  Kotlin, Swift, C#/.NET, and Go from 11 sequential 30-line full
  examples (~250 lines) into 7 `<details>`-wrapped compact snippets
  (~140 lines). Rust + Node + Python keep their full canonical
  examples (full lifecycle, cart/checkout, analytics — each shows a
  different use case worth seeing in default-open form). Readers get
  the "yes, the API ships in 10 bindings" proof on first paint
  without scrolling past 600 lines of near-duplicate code.
- **Key Features** section collapsed from 19 sub-headings + 130 lines
  of bulleted lists to a single 18-row capability matrix (~30 lines).
  Same coverage; A2A and VES v1.0 rows link out to depth docs since
  those are the differentiating primitives. Heading count dropped
  from 70 → 52.
- **Voice Mode** + **Multi-Provider AI** sections trimmed: removed
  ~10 lines of bulleted feature recapitulation that the capability
  matrix now covers; kept each section's unique CLI invocation
  examples + a single sentence anchoring the matrix link.
- **Domain Models** table collapsed from 21 rows / 25 lines to a
  4-line summary that names the 20 first-class domains and points
  readers at the OpenAPI spec + `crates/stateset-core/src/models/`
  for the authoritative inventory.
- **What's New in v1.0.4** section trimmed: 17-line release-notes
  block (with three sub-headings) replaced with a 4-line pointer at
  `CHANGELOG.md` for the full release history. Release notes are
  not README material — they belong in a versioned change log.
- **Database Schema (60 Tables)** flat list (22 lines, 10 domain
  groups × table-name dump) compressed to a 7-line summary that
  names the categories and points readers at
  `crates/stateset-db/migrations/` (authoritative DDL with indexes
  + FKs) and the dependency-direction guide.
- **Architecture** section explicitly preserved at full size — its
  dependency-direction graph, layer table, binding topology,
  operational surfaces, and onboarding order are unique
  information that doesn't live elsewhere in the README.

### ✓ Phase 5 — Bindings parity *(complete: 10/10 bindings wired)*

Cross-binding compatibility corpus at `bindings/test-vectors/v1.json`
(version 1, 14 vectors across 3 categories: canonical_json,
payload_plain_hash, merkle_root). Rust ground truth in
`crates/stateset-crypto/tests/cross_binding_vectors.rs` (4 tests).
Verifiers wired so far:

- **Node** (`bindings/node/test/cross-binding-vectors.js`) — 4 tests
  passing using napi exports `jcsCanonicalize` + `merkleRoot` and a
  composed `payloadPlainHash` (domain prefix locked to Rust).
- **Python** (`bindings/python/tests/test_cross_binding_vectors.py`) —
  4 tests passing using new pyfunctions `jcs_canonicalize`,
  `payload_plain_hash`, `merkle_root` exported from
  `bindings/python/src/lib.rs` (delegates to `stateset-crypto`).
- **Go** (`bindings/go/stateset/crypto_test.go`) — 4 tests passing
  using new cgo wrappers `JCSCanonicalize`, `PayloadPlainHash`,
  `MerkleRoot` over three new C-FFI exports in
  `bindings/go/src/lib.rs` (`stateset_crypto_*`). Cdylib built via
  `cargo build --release -p stateset-go`.
- **WASM** (`bindings/wasm/test/cross-binding-vectors.js`) — 4 tests
  passing using new wasm-bindgen exports `jcsCanonicalize`,
  `payloadPlainHash`, `merkleRoot` from `bindings/wasm/src/lib.rs`.
  Compiled via `wasm-pack build --release --target nodejs`. WASM
  consumes `stateset-crypto` with `default-features = false` to
  exclude the PQC deps.
- **Java** (`bindings/java/java/src/test/java/com/stateset/embedded/CryptoVectorTests.java`) —
  4 JUnit tests using new `Crypto.{jcsCanonicalize,payloadPlainHash,merkleRoot}`
  Java methods backed by three JNI exports
  (`Java_com_stateset_embedded_Crypto_native*`) in
  `bindings/java/src/lib.rs`. Verified in CI by the existing
  `jvm-bindings` job (which runs `gradle test`). Local env lacks
  JDK 11+/gradle, so verification is CI-only for now.
- **Kotlin** (`bindings/kotlin/kotlin/src/test/kotlin/com/stateset/embedded/CryptoVectorTest.kt`) —
  4 kotlin.test tests using new `Crypto` object (singleton) with three
  external-fun bridges to JNI exports
  (`Java_com_stateset_embedded_Crypto_native*`) in
  `bindings/kotlin/src/lib.rs`. Uses kotlinx-serialization-json (already
  a dep) for corpus parsing. Verified in the same `jvm-bindings`
  CI job (which runs `gradle test` from `bindings/kotlin/kotlin/`
  immediately after the Java tests). Local env lacks JDK 11+/gradle,
  so verification is CI-only for now.
- **.NET** (`bindings/dotnet/tests/CryptoVectorTests.cs`) — 4 xUnit
  tests using new public `Crypto` static class
  (`JcsCanonicalize`, `PayloadPlainHash`, `MerkleRoot`) wrapping
  P/Invoke calls into four new C-FFI exports
  (`stateset_crypto_*`) in `bindings/dotnet/src/lib.rs`. Uses
  `System.Text.Json` for corpus parsing (zero new package refs).
  Verified in CI by the existing `dotnet-bindings` job (which runs
  `dotnet test` after building the cdylib). Local env lacks
  dotnet, so verification is CI-only.
- **Swift** (`bindings/swift/Tests/StateSetTests/CryptoVectorTests.swift`) —
  4 XCTest tests using new public `Crypto` enum
  (`jcsCanonicalize`, `payloadPlainHash`, `merkleRoot`) wrapping
  the same four C-FFI exports added to `bindings/swift/src/lib.rs`
  (byte-identical FFI shape to Go/.NET). The C header
  `Sources/StateSetC/include/stateset.h` was extended with the four
  new declarations. Uses Apple `CryptoKit.SHA256` and
  `Foundation.JSONSerialization` (zero new SwiftPM deps). Verified
  in CI by the existing `swift-bindings` job on `macos-latest`.
  Local env has no Swift toolchain, so verification is CI-only.
- **Ruby** (`bindings/ruby/spec/crypto_vector_spec.rb`) — 4 rspec
  examples using new `StateSet::Crypto` Ruby module with
  singleton methods `jcs_canonicalize`, `payload_plain_hash`,
  `merkle_root` wired via three magnus functions in
  `bindings/ruby/src/runtime.rs`. Uses Ruby stdlib `JSON`/`Digest`
  for corpus parsing — zero new gem deps. Verified in CI by the
  existing `ruby-bindings` job (which runs `bundle exec rake`,
  invoking `compile` + `spec`). Local env lacks `ruby.h`/Ruby 3.0+
  needed for magnus 0.7, so verification is CI-only.
- **PHP** (`bindings/php/tests/CryptoVectorTest.php`) — 4 phpunit
  tests using new `StateSet\Crypto` static class
  (`jcsCanonicalize`, `payloadPlainHash`, `merkleRoot`) wired via
  `#[php_class]` + `#[php_impl]` in `bindings/php/src/runtime.rs`
  (ext-php-rs 0.13). Stub class declared in
  `bindings/php/stubs/StateSet.php` so the autoload check picks it
  up. CI's `php-bindings` job was extended: it now builds the
  native extension (`cargo build --features runtime --release`)
  and runs the parity test under
  `php -d extension=$PWD/target/release/libstateset_embedded.so` —
  upgrading PHP from "stub-only" to actually exercising the real
  ext-php-rs path. Local env has no PHP, so verification is CI-only.

**Phase 5 complete.** 10 binding implementations now consume the
same corpus end-to-end. Recipe documented in
`bindings/test-vectors/README.md` — read JSON, run binding primitives,
assert byte-equal hex.

**CI wired** (`.github/workflows/ci.yml`): each of the four binding
jobs (`node-bindings`, `python-bindings`, `go-bindings`,
`wasm-bindings`) now invokes its respective parity test as a step
right after the existing build/smoke. Rust ground truth is exercised
by `cargo test -p stateset-crypto` in the existing `rust` job. So a
breaking change to canonicalization, payload-hash, or merkle in
*any* of {Rust, Node, Python, Go, WASM} fails CI on push.

### ✓ Phase 1.2 — Postgres parity tests *(complete)*

The "untouched" label was a misclassification — Postgres parity is
already covered by **13 dedicated test files** exercised by a 3-way
CI matrix (`db-postgres`, `db-postgres-saga`, `embedded-postgres`)
running on `postgres:16` services. Drives:

- **`crates/stateset-db/tests/postgres_*.rs`** (10 files):
  `postgres_migrations`, `postgres_validations`,
  `postgres_order_transitions`, `postgres_order_versioning`,
  `postgres_crud`, `postgres_agent_cards`, `postgres_x402_credits`,
  `postgres_x402_payment_intents`, `postgres_guard`, `postgres_saga`.
- **`crates/stateset-embedded/tests/postgres_*.rs`** (3 files):
  `postgres_async_smoke`, `postgres_cart_checkout_smoke`,
  `postgres_x402_smoke`.
- Orchestrated by `scripts/ci/run_postgres_parity_matrix.sh` with
  `POSTGRES_PARITY_MODE` selector. The script exits cleanly under
  `DRY_RUN=1` and `cargo check -p stateset-db --features postgres`
  builds cleanly locally.
- Gated in CI via the `postgres:` job in `.github/workflows/ci.yml`
  (lines 546+) — three matrix entries, each with a `postgres:16`
  service container, `--features postgres` (and `,saga` for saga
  mode), and the parity script as the single test step.

Future expansion (modules without Postgres backends yet, e.g.
analytics, fulfillment, accounts_payable) is gated on the Postgres
storage adapter shipping for those domains, not on the test
infrastructure.

### Untouched

- **Phase 7** — Strategic stretch (PQC hard finality, SOC 2 Type I,
  formal verification of policy DSL + sync convergence).

## How to verify

```bash
# Rust tests (~3,800 across the workspace)
cargo test --workspace --no-fail-fast

# Property tests with deeper soak
PROPTEST_CASES=4096 cargo test --package stateset-policy --test proptest_operator
PROPTEST_CASES=4096 cargo test --package stateset-sync   --test proptest_conflict

# Admin component tests
cd admin && npm test

# Supply-chain audit
cargo vet --locked

# Fuzz one target locally (5-minute soak)
cd crates/stateset-crypto && cargo +nightly fuzz run canonicalize_json -- -max_total_time=300

# Verify a signed release (requires gh + cosign installed)
TAG=v1.0.4 REPO=stateset/stateset-icommerce
gh release download "$TAG" --repo "$REPO" \
  --pattern "stateset-icommerce-*.tar.gz" \
  --pattern "stateset-icommerce-*.SHA256SUMS*"
cosign verify-blob \
  --certificate "stateset-icommerce-${TAG}.SHA256SUMS.pem" \
  --signature   "stateset-icommerce-${TAG}.SHA256SUMS.sig" \
  --certificate-identity-regexp \
    "^https://github.com/${REPO}/\.github/workflows/release-sign\.yml@refs/tags/v.*$" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "stateset-icommerce-${TAG}.SHA256SUMS"
sha256sum -c "stateset-icommerce-${TAG}.SHA256SUMS"
```
