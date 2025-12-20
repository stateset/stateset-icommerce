# StateSet CLI Sync Implementation Requirements

## Implementation Requirements Document

**Feature:** VES Sync Commands for @stateset/cli
**Version:** 0.2.0
**Status:** Design
**Author:** StateSet Core Infrastructure
**Date:** December 2025

---

## 1. Executive Summary

This document specifies the implementation requirements for adding Verifiable Event Sync (VES) capabilities to the existing `@stateset/cli` package. The sync feature enables local SQLite databases to synchronize with the `stateset-sequencer` service, providing deterministic event ordering, conflict resolution, and cryptographic audit trails.

---

## 2. Current CLI Architecture

### 2.1 Existing Structure

```
@stateset/cli/
├── bin/
│   ├── stateset.js           # Main CLI entry point
│   ├── stateset-orders.js    # Orders specialist agent
│   ├── stateset-checkout.js  # Checkout flow agent
│   ├── stateset-inventory.js # Inventory agent
│   ├── stateset-returns.js   # Returns agent
│   ├── stateset-analytics.js # Analytics agent
│   ├── stateset-chat.js      # Chat interface
│   ├── stateset-direct.js    # Direct tool calls
│   └── stateset-create.js    # Scaffold generator
├── src/
│   ├── index.js              # Public exports
│   ├── claude-harness.js     # Agent SDK integration
│   ├── mcp-server.js         # MCP tool server
│   ├── permissions.js        # Permission gates
│   ├── telemetry.js          # Observability
│   ├── output.js             # Rich console output
│   └── scaffold-server.js    # Storefront generator
└── package.json
```

### 2.2 Key Dependencies

- `@anthropic-ai/claude-agent-sdk` - Agent orchestration
- `@stateset/embedded` - Local SQLite commerce engine
- `zod` - Schema validation

---

## 3. New Files to Implement

### 3.1 CLI Entry Points

#### `bin/stateset-sync.js`

Primary sync command entry point.

```javascript
#!/usr/bin/env node

/**
 * StateSet Sync Agent - VES synchronization commands
 *
 * Usage:
 *   stateset-sync push              # Push local events to sequencer
 *   stateset-sync pull              # Pull remote events and apply locally
 *   stateset-sync status            # Show sync state
 *   stateset-sync verify <event-id> # Verify event inclusion proof
 *   stateset-sync rebase            # Rebase after conflict
 *   stateset-sync init              # Initialize sync for this store
 */
```

**Commands:**

| Command | Description | Flags |
|---------|-------------|-------|
| `push` | Push pending local events to sequencer | `--batch-size`, `--dry-run` |
| `pull` | Pull and apply remote events | `--from`, `--limit` |
| `status` | Display sync status | `--json`, `--verbose` |
| `verify` | Verify event inclusion proof | `--event-id`, `--batch-id` |
| `rebase` | Rebase local state after conflict | `--force` |
| `init` | Initialize sync configuration | `--sequencer-url`, `--tenant-id` |
| `history` | Show sync history | `--limit` |

---

### 3.2 Core Sync Modules

#### `src/sync/index.js`

Public exports for sync functionality.

```javascript
export { SyncEngine, createSyncEngine } from './engine.js';
export { Outbox, createOutbox } from './outbox.js';
export { SequencerClient, createSequencerClient } from './client.js';
export { EventCapture, wrapCommerceWithEvents } from './capture.js';
export { ConflictResolver } from './conflict.js';
export { SyncConfig, loadSyncConfig, saveSyncConfig } from './config.js';
```

---

#### `src/sync/outbox.js`

Local event outbox management.

**Schema:**

```sql
CREATE TABLE IF NOT EXISTS _ves_outbox (
    local_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT UNIQUE NOT NULL,
    command_id TEXT,
    tenant_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    base_version INTEGER,
    source_agent TEXT NOT NULL,
    created_at TEXT NOT NULL,

    -- Sync tracking
    sync_status TEXT DEFAULT 'pending',  -- pending, synced, failed, rejected
    remote_sequence INTEGER,
    synced_at TEXT,
    rejection_reason TEXT,
    retry_count INTEGER DEFAULT 0,
    last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_outbox_status ON _ves_outbox(sync_status);
CREATE INDEX IF NOT EXISTS idx_outbox_entity ON _ves_outbox(entity_type, entity_id);
```

**Interface:**

```typescript
interface Outbox {
  // Write
  append(event: EventEnvelope): Promise<number>;  // Returns local_seq
  appendBatch(events: EventEnvelope[]): Promise<number[]>;

  // Read
  getPending(limit?: number): Promise<OutboxEvent[]>;
  getByEventId(eventId: string): Promise<OutboxEvent | null>;
  getByEntityId(entityType: string, entityId: string): Promise<OutboxEvent[]>;

  // Sync status
  markSynced(localSeq: number, remoteSeq: number): Promise<void>;
  markFailed(localSeq: number, error: string): Promise<void>;
  markRejected(localSeq: number, reason: string): Promise<void>;

  // Queries
  getStats(): Promise<OutboxStats>;
  getLastSyncedSequence(): Promise<number | null>;
  getPendingCount(): Promise<number>;

  // Maintenance
  pruneOldEvents(olderThanDays: number): Promise<number>;
  retryFailed(): Promise<number>;
}

interface OutboxEvent {
  localSeq: number;
  eventId: string;
  commandId?: string;
  tenantId: string;
  storeId: string;
  entityType: string;
  entityId: string;
  eventType: string;
  payload: object;
  payloadHash: string;
  baseVersion?: number;
  sourceAgent: string;
  createdAt: Date;
  syncStatus: 'pending' | 'synced' | 'failed' | 'rejected';
  remoteSequence?: number;
  syncedAt?: Date;
  rejectionReason?: string;
  retryCount: number;
  lastError?: string;
}

interface OutboxStats {
  total: number;
  pending: number;
  synced: number;
  failed: number;
  rejected: number;
  oldestPending?: Date;
  lastSynced?: Date;
}
```

---

#### `src/sync/client.js`

gRPC client for stateset-sequencer.

**Interface:**

```typescript
interface SequencerClient {
  // Connection
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  isConnected(): boolean;

  // Push operations
  push(batch: EventBatch): Promise<IngestReceipt>;
  pushWithRetry(batch: EventBatch, maxRetries?: number): Promise<IngestReceipt>;

  // Pull operations
  pull(fromSequence: number, limit?: number): Promise<SequencedEvent[]>;
  pullStream(fromSequence: number): AsyncIterable<SequencedEvent>;

  // State queries
  getHead(tenantId: string, storeId: string): Promise<SyncState>;
  getCommitment(batchId: string): Promise<BatchCommitment | null>;

  // Verification
  getInclusionProof(eventId: string): Promise<MerkleProof>;
  verifyInclusion(eventId: string, proof: MerkleProof, root: string): boolean;
}

interface SyncState {
  tenantId: string;
  storeId: string;
  headSequence: number;
  stateRoot: string;
  lastCommitment?: string;
  lastCommittedAt?: Date;
}

interface IngestReceipt {
  batchId: string;
  accepted: number;
  rejected: number;
  sequenceStart: number;
  sequenceEnd: number;
  rejections: Array<{
    eventId: string;
    reason: string;
  }>;
}
```

**Configuration:**

```typescript
interface SequencerConfig {
  url: string;                    // grpc://localhost:50051
  apiKey?: string;                // API key authentication
  jwt?: string;                   // JWT token authentication
  tenantId: string;
  storeId: string;
  agentId: string;
  timeout?: number;               // Request timeout (ms)
  retryPolicy?: {
    maxRetries: number;
    baseDelay: number;
    maxDelay: number;
  };
  tls?: {
    enabled: boolean;
    certPath?: string;
    insecure?: boolean;           // Skip verification (dev only)
  };
}
```

---

#### `src/sync/engine.js`

Orchestrates the sync cycle.

**Interface:**

```typescript
interface SyncEngine {
  // Lifecycle
  initialize(): Promise<void>;
  shutdown(): Promise<void>;

  // Manual sync
  push(): Promise<PushResult>;
  pull(): Promise<PullResult>;
  fullSync(): Promise<SyncResult>;

  // Status
  getStatus(): Promise<SyncStatus>;
  getHealth(): Promise<HealthStatus>;

  // Conflict handling
  hasConflicts(): Promise<boolean>;
  getConflicts(): Promise<ConflictInfo[]>;
  rebase(): Promise<RebaseResult>;

  // Background sync (optional)
  startBackgroundSync(intervalMs: number): void;
  stopBackgroundSync(): void;

  // Events
  on(event: 'push' | 'pull' | 'conflict' | 'error', handler: Function): void;
}

interface PushResult {
  success: boolean;
  pushed: number;
  rejected: number;
  receipt?: IngestReceipt;
  error?: string;
}

interface PullResult {
  success: boolean;
  pulled: number;
  applied: number;
  conflicts: number;
  error?: string;
}

interface SyncStatus {
  connected: boolean;
  localHead: number;
  remoteHead: number;
  pending: number;
  lag: number;                    // remoteHead - localHead
  lastPush?: Date;
  lastPull?: Date;
  conflicts: number;
}
```

---

#### `src/sync/capture.js`

Wraps commerce operations to emit events.

**Implementation Strategy:**

```javascript
/**
 * Wraps the Commerce instance to capture events on mutations.
 *
 * Every write operation (create, update, delete) atomically:
 * 1. Executes the original operation
 * 2. Appends an event to the outbox
 *
 * Read operations pass through unchanged.
 */
export function wrapCommerceWithEvents(commerce, config) {
  const outbox = createOutbox(commerce.db);

  return {
    ...commerce,

    orders: wrapResource(commerce.orders, 'order', outbox, config),
    customers: wrapResource(commerce.customers, 'customer', outbox, config),
    products: wrapResource(commerce.products, 'product', outbox, config),
    inventory: wrapResource(commerce.inventory, 'inventory', outbox, config),
    returns: wrapResource(commerce.returns, 'return', outbox, config),
    payments: wrapResource(commerce.payments, 'payment', outbox, config),
    carts: wrapResource(commerce.carts, 'cart', outbox, config),

    // Expose outbox for sync operations
    _outbox: outbox,
  };
}
```

**Event Type Mapping:**

| Commerce Method | Event Type |
|-----------------|------------|
| `orders.create()` | `order.created` |
| `orders.updateStatus()` | `order.status_changed` |
| `orders.ship()` | `order.shipped` |
| `orders.cancel()` | `order.cancelled` |
| `customers.create()` | `customer.created` |
| `customers.update()` | `customer.updated` |
| `products.create()` | `product.created` |
| `products.update()` | `product.updated` |
| `inventory.adjust()` | `inventory.adjusted` |
| `inventory.reserve()` | `inventory.reserved` |
| `inventory.release()` | `inventory.released` |
| `returns.create()` | `return.requested` |
| `returns.approve()` | `return.approved` |
| `returns.complete()` | `return.completed` |
| `payments.create()` | `payment.created` |
| `payments.markCompleted()` | `payment.completed` |
| `carts.create()` | `cart.created` |
| `carts.addItem()` | `cart.item_added` |
| `carts.checkout()` | `cart.checked_out` |

---

#### `src/sync/conflict.js`

Conflict detection and resolution.

**Interface:**

```typescript
interface ConflictResolver {
  // Detection
  detectConflicts(localEvents: OutboxEvent[], remoteEvents: SequencedEvent[]): ConflictInfo[];

  // Resolution strategies
  resolve(conflict: ConflictInfo, strategy: ResolutionStrategy): Promise<Resolution>;

  // Rebase
  rebase(localEvents: OutboxEvent[], remoteEvents: SequencedEvent[]): Promise<RebaseResult>;
}

interface ConflictInfo {
  type: 'version' | 'invariant' | 'concurrent';
  localEvent: OutboxEvent;
  remoteEvent?: SequencedEvent;
  entityType: string;
  entityId: string;
  description: string;
  suggestedStrategy: ResolutionStrategy;
}

type ResolutionStrategy =
  | 'remote-wins'      // Accept remote, discard local
  | 'local-wins'       // Re-push local with new base_version
  | 'merge'            // Combine changes (entity-specific logic)
  | 'manual';          // Require user intervention

interface RebaseResult {
  success: boolean;
  rebased: number;
  conflicts: number;
  unresolvedConflicts: ConflictInfo[];
}
```

---

#### `src/sync/config.js`

Sync configuration management.

**Config File Location:** `.stateset/sync.json`

```json
{
  "sequencer": {
    "url": "grpc://sequencer.stateset.io:443",
    "tls": true
  },
  "identity": {
    "tenantId": "tenant-uuid",
    "storeId": "store-uuid",
    "agentId": "agent-uuid"
  },
  "auth": {
    "apiKey": "sk_...",
    "jwt": null
  },
  "sync": {
    "autoSync": false,
    "syncIntervalMs": 30000,
    "batchSize": 100,
    "retryPolicy": {
      "maxRetries": 3,
      "baseDelay": 1000,
      "maxDelay": 30000
    }
  },
  "local": {
    "dbPath": "./store.db",
    "outboxRetentionDays": 30
  }
}
```

---

### 3.3 MCP Server Extensions

#### `src/sync-mcp-server.js`

MCP tools for sync operations (enables AI agent to manage sync).

**Tools:**

| Tool Name | Description |
|-----------|-------------|
| `sync_status` | Get current sync status |
| `sync_push` | Push pending events to sequencer |
| `sync_pull` | Pull and apply remote events |
| `sync_verify` | Verify event inclusion |
| `sync_conflicts` | List unresolved conflicts |
| `sync_rebase` | Rebase local state |

---

## 4. Package.json Updates

```json
{
  "name": "@stateset/cli",
  "version": "0.1.5",
  "bin": {
    "stateset": "./bin/stateset.js",
    "stateset-sync": "./bin/stateset-sync.js",
    ...
  },
  "dependencies": {
    "@anthropic-ai/claude-agent-sdk": "^0.1.70",
    "@stateset/embedded": "^0.1.5",
    "@grpc/grpc-js": "^1.9.0",
    "@grpc/proto-loader": "^0.7.0",
    "zod": "^3.25.76"
  }
}
```

---

## 5. CLI Command Specifications

### 5.1 `stateset-sync init`

Initialize sync for a local database.

```bash
stateset-sync init \
  --sequencer-url grpc://sequencer.stateset.network:443 \
  --tenant-id <uuid> \
  --store-id <uuid> \
  --api-key <key>
```

**Behavior:**
1. Validates connection to sequencer
2. Creates `.stateset/sync.json` config file
3. Creates `_ves_outbox` table in SQLite
4. Registers agent with sequencer (optional)
5. Performs initial sync (pull current state)

**Output:**
```
✓ Connected to sequencer at grpc://sequencer.stateset.network:443
✓ Created sync configuration at .stateset/sync.json
✓ Initialized outbox table in ./store.db
✓ Registered agent: agent-abc123
✓ Initial sync complete (pulled 0 events)

Sync is ready! Run 'stateset-sync status' to check state.
```

---

### 5.2 `stateset-sync push`

Push pending local events to sequencer.

```bash
stateset-sync push [options]

Options:
  --batch-size <n>   Max events per batch (default: 100)
  --dry-run          Show what would be pushed without pushing
  --force            Push even if there are conflicts
  --verbose          Show detailed progress
```

**Output:**
```
📤 Pushing events to sequencer...

  Pending events: 15
  Batch size:     100

  [████████████████████████████████████████] 100%

✓ Push complete

  Accepted:  14
  Rejected:  1

  Rejections:
    - evt_abc123: Duplicate event_id

  Sequence range: 1042 → 1055
  Batch ID:       batch_xyz789
```

---

### 5.3 `stateset-sync pull`

Pull remote events and apply locally.

```bash
stateset-sync pull [options]

Options:
  --from <seq>    Start from sequence number (default: last synced)
  --limit <n>     Max events to pull (default: unlimited)
  --dry-run       Show what would be applied without applying
  --verbose       Show each event being applied
```

**Output:**
```
📥 Pulling events from sequencer...

  Local head:   1020
  Remote head:  1055
  Gap:          35 events

  [████████████████████████████████████████] 100%

✓ Pull complete

  Pulled:   35
  Applied:  35
  Conflicts: 0

  New local head: 1055
```

---

### 5.4 `stateset-sync status`

Display sync status.

```bash
stateset-sync status [options]

Options:
  --json      Output as JSON
  --verbose   Show detailed stats
```

**Output:**
```
🔄 Sync Status

  Connection:     ✓ Connected to grpc://sequencer.stateset.com:443

  Local State:
    Database:     ./store.db
    Outbox:       15 pending, 1,024 synced, 2 failed
    Local head:   1020

  Remote State:
    Remote head:  1055
    State root:   0x7f3a...b2c4
    Last commit:  2025-12-19T14:30:00Z

  Sync Gap:       35 events behind

  Last Activity:
    Last push:    2025-12-19T14:25:00Z (5 min ago)
    Last pull:    2025-12-19T14:20:00Z (10 min ago)

  Health:         ⚠ Sync lag detected (35 events)
```

---

### 5.5 `stateset-sync verify`

Verify event inclusion in commitment.

```bash
stateset-sync verify <event-id> [options]

Options:
  --batch-id <id>   Verify against specific batch
  --verbose         Show proof details
```

**Output:**
```
🔍 Verifying event: evt_abc123

  Event Details:
    Type:         order.created
    Entity:       order/ord-456
    Sequence:     1042
    Payload hash: 0x3f2a...c8d1

  Commitment:
    Batch ID:     batch_xyz789
    Events root:  0x8e4b...a1f2
    State root:   0x7f3a...b2c4
    On-chain tx:  0x9c5d...e3f4

  Proof:
    Path length:  8
    Verification: ✓ Valid

✓ Event inclusion verified against on-chain commitment
```

---

### 5.6 `stateset-sync rebase`

Rebase local state after conflict.

```bash
stateset-sync rebase [options]

Options:
  --force      Discard local changes on conflict (remote wins)
  --dry-run    Show what would happen without applying
  --verbose    Show detailed rebase steps
```

**Output:**
```
🔄 Rebasing local state...

  Conflicts detected: 2

  Conflict 1: order/ord-123
    Local:  order.status_changed (pending → shipped)
    Remote: order.status_changed (pending → cancelled)
    Strategy: remote-wins (--force)

  Conflict 2: inventory/inv-456
    Local:  inventory.adjusted (-5)
    Remote: inventory.adjusted (-3)
    Strategy: merge (sum adjustments)

  Rebasing...

✓ Rebase complete

  Events rebased:   12
  Conflicts resolved: 2
  New local head:   1055
```

---

## 6. Integration with Existing CLI

### 6.1 claude-harness.js Modifications

Add sync-aware commerce wrapper:

```javascript
// In runAgentLoop()
const commerce = new Commerce(dbPath);

// If sync is configured, wrap with event capture
const syncConfig = loadSyncConfig();
const wrappedCommerce = syncConfig
  ? wrapCommerceWithEvents(commerce, syncConfig)
  : commerce;

// Pass wrapped commerce to MCP server
const mcpServer = createStatesetMcpServer({
  commerce: wrappedCommerce,
  ...
});
```

### 6.2 New Flags for Main CLI

```bash
stateset [options] "<request>"

New Options:
  --sync           Enable auto-sync after mutations
  --sync-push      Push after this command completes
  --sync-status    Show sync status before executing
```

---

## 7. Testing Requirements

### 7.1 Unit Tests

| Test File | Coverage |
|-----------|----------|
| `test/sync/outbox.test.js` | Outbox CRUD, status transitions |
| `test/sync/client.test.js` | gRPC client mocking |
| `test/sync/engine.test.js` | Sync orchestration |
| `test/sync/capture.test.js` | Event capture for each operation |
| `test/sync/conflict.test.js` | Conflict detection and resolution |
| `test/sync/config.test.js` | Config loading/saving |

### 7.2 Integration Tests

| Test | Description |
|------|-------------|
| `test/integration/push-pull.test.js` | Full push/pull cycle with real sequencer |
| `test/integration/conflict.test.js` | Conflict scenarios and rebase |
| `test/integration/offline.test.js` | Offline operation and reconnection |

### 7.3 E2E Tests

| Test | Description |
|------|-------------|
| `test/e2e/cli-sync.test.js` | CLI command execution |
| `test/e2e/agent-sync.test.js` | AI agent using sync tools |

---

## 8. Error Handling

### 8.1 Error Codes

| Code | Name | Description |
|------|------|-------------|
| `SYNC_001` | `CONNECTION_FAILED` | Cannot connect to sequencer |
| `SYNC_002` | `AUTH_FAILED` | Authentication failed |
| `SYNC_003` | `PUSH_REJECTED` | Batch rejected by sequencer |
| `SYNC_004` | `CONFLICT_DETECTED` | Version conflict on push |
| `SYNC_005` | `PULL_FAILED` | Failed to pull events |
| `SYNC_006` | `APPLY_FAILED` | Failed to apply remote event |
| `SYNC_007` | `REBASE_FAILED` | Rebase could not complete |
| `SYNC_008` | `CONFIG_INVALID` | Invalid sync configuration |
| `SYNC_009` | `OUTBOX_FULL` | Outbox exceeded size limit |
| `SYNC_010` | `PROOF_INVALID` | Inclusion proof verification failed |

### 8.2 Recovery Strategies

| Error | Recovery |
|-------|----------|
| `CONNECTION_FAILED` | Retry with exponential backoff |
| `AUTH_FAILED` | Prompt for new credentials |
| `PUSH_REJECTED` | Show rejection reasons, retry valid events |
| `CONFLICT_DETECTED` | Prompt for rebase or manual resolution |
| `APPLY_FAILED` | Log and continue, mark event as failed |

---

## 9. Security Considerations

### 9.1 Credential Storage

- API keys stored in `.stateset/sync.json` (gitignored)
- Support for environment variables: `STATESET_API_KEY`, `STATESET_JWT`
- Support for credential helpers (future)

### 9.2 Data in Transit

- TLS required for production sequencer connections
- Event payloads optionally encrypted with tenant key

### 9.3 Local Storage

- Outbox contains sensitive commerce data
- Recommend SQLCipher for local database encryption

---

## 10. Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Outbox schema and basic operations
- [ ] Sync config management
- [ ] `stateset-sync init` command
- [ ] `stateset-sync status` command

### Phase 2: Push/Pull (Week 2-3)
- [ ] gRPC client implementation
- [ ] `stateset-sync push` command
- [ ] `stateset-sync pull` command
- [ ] Basic error handling

### Phase 3: Event Capture (Week 3-4)
- [ ] Commerce wrapper for event capture
- [ ] Event type mapping for all operations
- [ ] Integration with claude-harness.js
- [ ] Unit tests for capture

### Phase 4: Conflict Resolution (Week 4-5)
- [ ] Conflict detection logic
- [ ] `stateset-sync rebase` command
- [ ] Resolution strategies
- [ ] Integration tests

### Phase 5: Verification & Polish (Week 5-6)
- [ ] `stateset-sync verify` command
- [ ] MCP tools for sync
- [ ] E2E tests
- [ ] Documentation

---

## 11. Success Metrics

| Metric | Target |
|--------|--------|
| Push latency (p99) | < 5s for 100 events |
| Pull latency (p99) | < 10s for 1000 events |
| Conflict resolution rate | > 95% auto-resolved |
| CLI command success rate | > 99% |
| Test coverage | > 80% |

---

## 12. Dependencies on stateset-sequencer

The CLI sync implementation requires the following from the Rust sequencer:

| Feature | Status | Sequencer Component |
|---------|--------|---------------------|
| gRPC Push endpoint | ✅ Done | `grpc/service.rs` |
| gRPC Pull endpoint | ✅ Done | `grpc/service.rs` |
| GetHead endpoint | ✅ Done | `grpc/service.rs` |
| GetInclusionProof | ✅ Done | `grpc/service.rs` |
| Event deduplication | ✅ Done | `infra/postgres/sequencer.rs` |
| Conflict detection | ✅ Done | `projection/mod.rs` |
| Proto definitions | ✅ Done | `proto/sequencer.proto` |

---

## Appendix A: Event Envelope Schema

```typescript
interface EventEnvelope {
  event_id: string;           // UUID v4
  command_id?: string;        // Idempotency key
  tenant_id: string;
  store_id: string;
  entity_type: string;        // order, customer, product, etc.
  entity_id: string;
  event_type: string;         // order.created, customer.updated, etc.
  payload: object;            // Domain-specific event data
  payload_hash: string;       // SHA256 of canonical JSON payload
  base_version?: number;      // For optimistic concurrency
  created_at: string;         // ISO 8601
  source_agent: string;       // Agent that created the event
}
```

---

## Appendix B: Proto Definition Reference

See `stateset-sequencer/proto/sequencer.proto` for gRPC service definitions.

Key messages:
- `PushRequest` / `PushResponse`
- `PullRequest` / `PullResponse`
- `GetHeadRequest` / `GetHeadResponse`
- `GetInclusionProofRequest` / `GetInclusionProofResponse`
- `EventEnvelope`
- `SequencedEvent`
- `IngestReceipt`
- `MerkleProof`
