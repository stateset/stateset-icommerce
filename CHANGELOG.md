# Changelog

All notable changes to StateSet iCommerce will be documented in this file.

This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

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
