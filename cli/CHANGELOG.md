# Changelog

All notable changes to `@stateset/cli` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.17] - 2026-03-02

### Added
- A2A Rust crates elevation: 6 new modules (negotiation, task delegation, capability discovery, trust verification, message routing, protocol handshake)
- OpenAPI 3.1 spec generation via utoipa (`GET /api/v1/openapi.json`) with full schema coverage for 19 REST endpoints
- Technical whitepaper for StateSet iCommerce platform
- Token bucket HTTP rate limiting middleware (no new deps, configurable RPS + burst)
- Centralized env validation (`cli/src/env.js`) — Zod-validated schema for 40+ environment variables
- `#[tracing::instrument]` on 19 HTTP route handlers for production observability
- `#[deny(unsafe_code)]` on `stateset-crypto` and `stateset-macros` crates
- `cargo deny` policy: licenses, advisories, bans (openssl via native-tls wrappers only), sources

### Fixed
- 22 silent catch blocks across 14 CLI files — all now log with `console.debug()`
- 17 timer leaks across 14 CLI files — stored refs + `.unref()` on long-lived intervals
- deny.toml duplicate `[advisories]` section merged

## [0.7.15] - 2026-02-28

### Added
- A-grade Rust crate elevation: 226 new integration tests (pricing, policy, authz, a2a, http)
- 21 property-based tests (pricing + crypto) via proptest
- `[lints] workspace = true` enforced on all binding crates

### Fixed
- ~200+ clippy warnings eliminated: `use_self`, `missing_const_for_fn`, `doc_markdown`, `redundant_clone`
- Zero clippy warnings on `cargo clippy --all-targets`

## [0.7.14] - 2026-02-27

### Added
- Agentic hardening round: `.int()`, `.positive()`, `.min()`, `.max()` on 100+ Zod string ID fields
- `#[must_use]` on Money arithmetic, ID types, ForecastingEngine, LogEntry builder
- Magic number extraction to named constants across 7 CLI files
- ~110 `console.log` migrated to `console.info`/`debug`/`warn` across 21 production files

## [0.7.12] - 2026-02-26

### Added
- Production hardening: state machine methods (`can_transition_to`, `is_terminal`) on 5 status enums
- Atomic database transactions for payment operations
- Crypto hardening: `zeroize` on keys, `subtle::ConstantTimeEq` for hash comparison
- Money safety: `checked_mul_scalar`, `checked_div_scalar`, `abs`, `negate`, fixed `is_negative` for -0
- 9 agentic commerce observability counters + `LatencyHistogram` (p50/p95/p99)
- 7 new test fixture builders (payment, shipment, return, subscription, cart, warranty)

## [0.7.9] - 2026-02-22

### Fixed
- Webhook SSRF hardening, crypto decode safety, ID derive macro fixes
- Removed panic paths in db/jobs/pricing behavior
- FFI safety hardening, HTTP readiness contract fixes
- A2A validation and protocol integrity semantics

## [0.7.5] - 2026-02-18

### Added
- Env validation test sandbox safety
- Lifecycle, config, and update workflow hardening
- Sync, scheduler, HTTP, and migrator updates

## [0.7.0] - 2026-02-15

### Added
- Typed IDs with SQLx encoding support
- WASM systems integration
- Financial crates with test coverage
- FFI, HTTP, jobs, and migrations crate updates

### Changed
- API call sites updated for typed ID system

## [0.6.0] - 2026-02-10

### Added
- Standalone iCommerce adoption wedge
- Stripe adapter with webhook verification
- WooCommerce adapter with template scaffolding
- Policy engine with file watcher and YAML authoring
- `configure_stripe` and `configure_woocommerce` MCP tools
- `--quickstart` flag for zero-config setup
- 607 new tests across adapters and policy engine

## [0.5.0] - 2026-02-02

### Changed
- Bumped CLI to 0.5.0 and synced versioned defaults/docs.

## [0.4.0] - 2026-01-30

### Added

#### Lane-Based Command Queue (`src/command-queue.js`)
Session-based serialization architecture inspired by Clawdbot:
- **Serial Lanes** - Operations within a session execute one at a time, preventing race conditions
- **Parallel Lanes** - Background tasks (cron, batch) run concurrently with configurable limits
- **Queue Statistics** - Track queue depth, latency, and throughput per lane
- **Idle Cleanup** - Automatic cleanup of stale lanes to prevent memory leaks
- Default to serial, go parallel explicitly

#### Context Window Guard (`src/context-guard.js`)
Proactive context management to prevent overflow failures:
- **Token Estimation** - Claude-like token counting using cl100k_base approximation
- **Pre-flight Checks** - Validate context usage before each LLM call
- **Auto-Compaction** - Summarize old messages when approaching 80% capacity
- **Conversation Summarizer** - Extract key facts and intents while reducing tokens
- Thresholds: 70% warn, 80% compact, 95% abort

#### Model Fallback Chain (`src/model-fallback.js`)
Automatic model failover for resilience:
- **Default Chain** - Claude Sonnet → Haiku → OpenAI GPT-4o-mini → Gemini Flash
- **Cooldown Tracking** - Exponential backoff per model/key on failures
- **Rate Limit Detection** - Pattern matching for 429, quota, capacity errors
- **Capability Filtering** - Only use models with required capabilities (tools, thinking)
- Callbacks for fallback and cooldown events

#### Markdown Memory Store (`src/memory/markdown-store.js`)
Transparent, human-readable memory system:
- **Main Memory** - `~/.stateset/memory/MEMORY.md` with auto-summarized entries
- **Session Memory** - Per-session transcripts in `sessions/{id}.md`
- **Entity Memory** - Entity-specific memories in `entities/{type}_{id}.md`
- **Topic Memory** - Knowledge by topic in `topics/{topic}.md`
- Human-readable, git-friendly, easy to debug and inspect

#### Unified Memory Module (`src/memory/index.js`)
- `UnifiedMemory` class that writes to both SQLite and Markdown stores
- Combined search across all memory backends
- Singleton access via `getUnifiedMemory()`

#### Semantic Browser Snapshots (`src/browser/browser-tools.js`)
Lightweight page representation for LLM agents:
- `getAccessibilityTree()` - Full ARIA accessibility tree via CDP
- `getSemanticSnapshot()` - Compact text format optimized for LLMs
- `interactByRef()` - Interact with elements by reference ID
- ~100x smaller than screenshots, dramatically reduces token cost
- Output format: `- button "Sign In" [ref=1]`

#### New Agent Loop Options
- `enableFallback` - Enable automatic model failover (default: true)
- `enableContextGuard` - Enable context window checking (default: true)
- `enableMemory` - Enable memory persistence (default: true)
- `useMarkdownMemory` - Use markdown memory store (default: true)
- `conversationHistory` - Existing history for context
- `onContextWarning` - Callback when context approaches limit
- `onFallback` - Callback when model fallback occurs

#### New Functions
- `runAgentLoopQueued()` - Queue-wrapped agent execution with session serialization
- `runAgentLoopParallel()` - Parallel execution for batch processing
- `getQueueStats()` - Get command queue statistics

#### New Return Fields
- `usedModel` - Actual model used (may differ if fallback occurred)
- `fallbackAttempts` - Array of fallback attempts with success/failure
- `contextGuard` - Context usage info `{ action, usage }`

### Changed

- Agent loop now automatically saves to both SQLite and Markdown memory stores
- Context is checked and potentially compacted before each LLM call
- Model failures trigger automatic fallback with cooldown tracking
- All new modules exported from `claude-harness.js` for convenience

### Technical Notes

#### Architecture Improvements
These changes align with Clawdbot's architecture principles:
- "Default to Serial, go for Parallel explicitly" - Lane-based queuing
- Simple, inspectable memory - Markdown files alongside SQLite
- Proactive context management - Guard before call, not after failure
- Graceful degradation - Fallback chain with cooldown tracking

#### Memory Structure
```
~/.stateset/memory/
├── MEMORY.md           # Main memory (auto-summarized)
├── sessions/
│   └── {sessionId}.md  # Per-session transcripts
├── entities/
│   └── {type}_{id}.md  # Entity-specific (customer, order)
└── topics/
    └── {topic}.md      # Topic-specific knowledge
```

#### Fallback Chain
```
Claude Sonnet (primary)
    ↓ rate limit / error
Claude Haiku (fast fallback)
    ↓ rate limit / error
OpenAI GPT-4o-mini (cross-provider)
    ↓ rate limit / error
Gemini Flash (last resort)
```

## [0.2.4] - 2026-01-26

### Added
- Vector search tooling for embedding and similarity workflows.

## [0.2.0] - 2026-01-11

### Added

#### Verifiable Event Sync (VES) Protocol v1.0
A complete event sourcing system with cryptographic verification for multi-agent synchronization:

- **Ed25519 Key Management** (`src/sync/keys.js`)
  - Agent keypair generation and secure storage
  - VES-compliant key derivation (HKDF-SHA256)
  - Key rotation policies with configurable schedules
  - Automatic key backup and recovery

- **Cryptographic Verification** (`src/sync/crypto.js`)
  - Domain-separated hashing with `VES_PAYLOAD_PLAIN_V1` prefix
  - Canonical JSON serialization (RFC 8785) for deterministic hashing
  - Ed25519 signature generation and verification
  - Legacy payload hash support for backwards compatibility
  - `computePayloadHash()`, `computeLegacyPayloadHash()`, `signEvent()`, `verifySignature()`

- **Event Outbox** (`src/sync/outbox.js`)
  - SQLite-backed event storage with better-sqlite3
  - Sequence number tracking for ordering
  - Push/pull state management
  - Conflict detection at entity level

- **Sync Engine** (`src/sync/engine.js`)
  - Push/pull orchestration with configurable intervals
  - Event subscription by entity type/ID
  - Automatic conflict resolution strategies
  - Real-time event streaming via callbacks

- **Conflict Resolution** (`src/sync/conflict.js`)
  - Optimistic concurrency control with base_version
  - Last-write-wins and custom merge strategies
  - Conflict event generation for audit trails

#### gRPC Bidirectional Streaming
Real-time synchronization with the StateSet Sequencer:

- **gRPC Client** (`src/sync/grpc-client.js`)
  - Bidirectional streaming for push/pull operations
  - Entity subscription for filtered real-time updates
  - Automatic reconnection with exponential backoff
  - Proto-based message serialization
  - Agent key registration and revocation
  - Sync state queries (head sequence, checkpoint)

- **Unified Client** (`src/sync/unified-client.js`)
  - Abstraction layer supporting both HTTP and gRPC transports
  - Automatic transport selection based on configuration
  - Consistent API across both protocols

- **Proto Definitions** (`src/sync/proto/`)
  - `sequencer.proto` - Event ingestion and retrieval
  - `sync.proto` - Bidirectional sync streams
  - `keys.proto` - Agent key management

#### Multi-Chain Stablecoin Payments
Native cryptocurrency payment support across 8 blockchain networks:

- **Supported Chains** (`src/chains/config.js`)
  - Solana (mainnet/devnet) - USDC
  - SET Chain - ssUSD (yield-bearing stablecoin)
  - Base L2 - USDC
  - Ethereum - USDC, USDT, DAI
  - Arbitrum L2 - USDC
  - Zcash (mainnet/testnet) - ZEC (t-addresses)
  - Bitcoin (mainnet/testnet) - BTC

- **Wallet Derivation** (`src/chains/wallet.js`)
  - VES Ed25519 seed to chain-specific wallet derivation
  - Ed25519 wallets for Solana/SET Chain
  - secp256k1 wallets for EVM chains, Bitcoin, Zcash
  - Deterministic address generation per agent/chain

- **Payment Operations** (`src/chains/stablecoin.js`)
  - Balance checking across all supported chains
  - Transaction building with proper encoding
  - Signature generation (simulation mode)
  - Transaction submission with confirmation tracking

- **Address Validation** (`src/chains/validation.js`)
  - Chain-specific address format validation
  - Checksum verification (EVM, Bitcoin, Zcash)
  - Base58/Bech32/Hex encoding support

- **Cryptographic Utilities** (`src/chains/crypto-utils.js`)
  - RIPEMD-160 implementation for Bitcoin/Zcash
  - SHA256 double hashing
  - secp256k1 utilities

#### New CLI Commands

- **`stateset-pay`** - Native stablecoin payment interface
  ```bash
  stateset pay --chains                    # List supported chains
  stateset pay --wallet --chain solana     # Show wallet address
  stateset pay --balance --chain solana    # Check balance
  stateset pay --apply --to <addr> --amount 50 --chain solana  # Send payment
  ```

- **`stateset-sync`** - Event synchronization CLI
  ```bash
  stateset sync push            # Push local events to sequencer
  stateset sync pull            # Pull new events from sequencer
  stateset sync status          # Show sync state
  stateset sync stream          # Start real-time sync stream
  ```

- **`stateset-autonomous`** - Autonomous agent operations
  ```bash
  stateset autonomous start     # Start autonomous mode
  stateset autonomous status    # Check agent status
  ```

#### New MCP Tools

- **commerce-stablecoin** (4 tools)
  - `get_agent_wallet` - Get wallet address for a blockchain
  - `get_wallet_balance` - Check stablecoin balance
  - `create_stablecoin_payment` - Send payment (requires --apply)
  - `list_supported_chains` - List available blockchains

### Changed

- **gRPC as Optional Dependency** - `@grpc/grpc-js` and `@grpc/proto-loader` are now optional dependencies, allowing installation without gRPC support for environments that don't need real-time sync.

- **SQLite Improvements** - Enhanced better-sqlite3 implementations with proper type handling, WAL mode, and optimized prepared statements.

- **Reconnection Logic** - Improved exponential backoff with jitter for gRPC reconnections, plus intentional disconnect detection to prevent unnecessary reconnection attempts.

### Fixed

- **Payload Hash Mismatch** - Fixed hash computation to use legacy format (no domain prefix) when communicating with the sequencer's gRPC API, maintaining compatibility with existing infrastructure.

- **Buffer/Hex Conversion** - Fixed `0x` prefix handling when converting between hex strings and buffers in outbox storage and retrieval.

- **Event Field Mapping** - Corrected field mapping when storing pulled events to SQLite, ensuring all VES fields are properly persisted.

- **Reconnection NaN Delay** - Fixed undefined retry policy values causing NaN delays during gRPC reconnection by adding proper nullish coalescing defaults.

### Technical Notes

#### VES Protocol Compatibility
The VES (Verifiable Event Sync) Protocol v1.0 provides cryptographic guarantees for event integrity:
- Events are signed with Ed25519 keys
- Payload hashes use domain-separated SHA256
- Base version tracking enables optimistic concurrency
- Sequence numbers ensure global ordering

#### Chain Integration Architecture
```
VES Ed25519 Seed (32 bytes)
       ↓ HKDF-SHA256 with chain-specific info
Chain-Specific Private Key
       ↓ Curve-specific derivation
Public Key → Address
```

#### Migration Notes
If upgrading from v0.1.x with an existing sync database, the outbox schema remains compatible. New VES fields will be populated on first sync.

## [0.1.7] - 2025-12-20

### Added

#### Payments API (5 tools)
Complete payment processing and refund management:
- `list_payments` - List all payments with filtering
- `get_payment` - Get payment details by ID
- `create_payment` - Create a payment for an order
- `complete_payment` - Mark payment as completed/captured
- `create_refund` - Process refunds for payments

#### Shipments API (3 tools)
Track shipments from warehouse to customer:
- `list_shipments` - List all shipments
- `create_shipment` - Create shipment with carrier and tracking info
- `deliver_shipment` - Mark shipment as delivered

#### Suppliers & Purchase Orders API (6 tools)
Full supply chain management:
- `list_suppliers` - List all suppliers
- `create_supplier` - Add new supplier with contact info
- `list_purchase_orders` - List all purchase orders
- `create_purchase_order` - Create PO for supplier
- `approve_purchase_order` - Approve PO for sending
- `send_purchase_order` - Send PO to supplier

#### Invoices API (5 tools)
B2B invoicing and accounts receivable:
- `list_invoices` - List all invoices
- `create_invoice` - Create invoice for customer
- `send_invoice` - Send invoice to customer
- `record_invoice_payment` - Record payment received on invoice
- `get_overdue_invoices` - Get overdue invoices for follow-up

#### Warranties API (4 tools)
Product warranty and claims management:
- `list_warranties` - List all warranties
- `create_warranty` - Create warranty for customer/product
- `create_warranty_claim` - File a warranty claim
- `approve_warranty_claim` - Approve warranty claim for processing

#### Manufacturing API (11 tools)
Bills of Materials and Work Order management:
- `list_boms` - List all Bills of Materials
- `get_bom` - Get BOM details with components
- `create_bom` - Create new BOM for a product
- `add_bom_component` - Add component/ingredient to BOM
- `activate_bom` - Activate BOM for production use
- `list_work_orders` - List manufacturing work orders
- `get_work_order` - Get work order details
- `create_work_order` - Create work order from BOM
- `start_work_order` - Start production on work order
- `complete_work_order` - Complete with quantity produced
- `cancel_work_order` - Cancel work order

### Fixed

- **Returns Schema**: Added missing `version` column to returns table in `012_versioning.sql` migration. This fixes the "column version does not exist" error when creating returns.
- **Invoice Payment Recording**: Fixed `record_invoice_payment` tool parameter name from `method` to `paymentMethod` to match the Rust binding interface. Fixed type conversion for amount field.
- **Warranty Creation**: Added required `customerId` parameter to `create_warranty` tool. Made `orderId` and `productId` optional to match the API contract.

### Changed

- Updated `TOOL_NAMES` array with all 34 new tool names for proper MCP registration
- Added new read-only tools to permission whitelist for safe preview mode operation
- Total MCP tools increased from 53 to **87 tools**

### Tool Count by Category

| Category | Tools |
|----------|-------|
| Customers | 3 |
| Orders | 6 |
| Products | 4 |
| Inventory | 6 |
| Returns | 5 |
| Carts/Checkout (ACP) | 14 |
| Analytics | 10 |
| Currency | 8 |
| Tax | 9 |
| Promotions | 10 |
| Subscriptions | 15 |
| Sync | 9 |
| Manufacturing | 11 |
| Payments | 5 |
| Shipments | 3 |
| Suppliers/POs | 6 |
| Invoices | 5 |
| Warranties | 4 |
| **Total** | **87** |

### Migration Notes

If upgrading from v0.1.6 or earlier with an existing database, run:

```sql
ALTER TABLE returns ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
```

## [0.1.2] - 2025-12-18

### Added

#### Storefront Creation Agent
- New `stateset-create` CLI command for scaffolding e-commerce websites
- Storefront agent with 13 scaffolding tools:
  - `create_project` - Initialize new storefront projects
  - `add_page` - Add pages (products, cart, checkout, account)
  - `add_component` - Add components (ProductCard, AddToCart, etc.)
  - `add_hook` - Add React hooks (useCart, useProducts, etc.)
  - `add_api_route` - Add API routes
  - `write_file` / `read_file` / `list_files` - File operations
  - `run_command` - Execute npm commands
  - `seed_database` - Create sample products
- Four project templates:
  - `nextjs` - Full-stack Next.js 14 with App Router, SSR, Tailwind
  - `nextjs-minimal` - Minimal Next.js setup
  - `vite-react` - Client-side SPA with WASM
  - `astro` - Static-first with Islands
- Comprehensive skill document with page, component, and hook templates
- Auto-routing to storefront agent for store creation requests

#### Observability & Telemetry
- New `telemetry.js` module with structured logging and tracing
- Distributed tracing with trace IDs and spans
- Tool call metrics with duration tracking
- Execution summary statistics
- `--verbose` flag for real-time telemetry output
- `--stats` flag to show execution statistics

#### Rich Output Formatting
- New `output.js` module for formatted CLI output
- ASCII table formatting with column alignment
- Progress bars and status indicators
- Currency, number, and date formatting
- Order/Cart/Customer card displays
- Color-coded status badges
- Consistent tool call formatting

#### Fine-Grained Permissions
- New `permissions.js` module for access control
- Five permission levels: `none`, `read`, `preview`, `write`, `admin`
- Per-tool permission mapping (56+ tools)
- Spending limits (max order value, daily totals)
- Rate limiting (tool calls/minute, write ops/minute)
- Confirmation thresholds for high-value operations
- Audit logging with sanitized parameters

#### Agent Improvements
- Enhanced agent routing with confidence scoring
- `routeToAgentWithConfidence()` returns confidence scores and alternatives
- Ambiguity detection for routing decisions

### Changed
- All CLI binaries now support `--verbose` flag
- Chat mode supports `/verbose on|off` command
- JSON output includes telemetry data when `--stats` is set
- Improved error messages with status icons

### Fixed
- Consistent version numbers across all CLI binaries

## [0.1.1] - 2025-12-17

### Added
- Initial release with core commerce agents
- Customer service, checkout, orders, inventory, returns, analytics agents
- 56 MCP tools for commerce operations
- Multi-currency support
- Interactive chat mode

## [0.1.0] - 2025-12-16

### Added
- Initial development release
