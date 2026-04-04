# Changelog

All notable changes to StateSet iCommerce will be documented in this file.

This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

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
