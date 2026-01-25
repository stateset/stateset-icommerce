# Phase 3: Conversation Context & Better Transactions Summary

## Overview
This phase addresses the three critical gaps identified for production deployment:
1. **Conversation Context Management** - Track agent operations across sessions
2. **Better Transactions (Saga Pattern)** - Atomic multi-step operations with rollback
3. **Session Persistence** - Save and resume agent conversation state

## Files Created

### 1. Conversation Context Management
**File:** `/home/dom/stateset-icommerce/cli/src/mcp-conversation-context.js`

**Purpose:** Track all tool calls in an agent session for context-aware interactions

**Key Features:**
- Tool call history with success/failure tracking
- Operation timeline with causality tracking
- Automatic rollback of failed operations
- Context-aware error messages
- Next action suggestions based on history

**API:**
```javascript
// Create new context
const context = new ConversationContext(commerce, sessionId);

// Record tool execution
context.recordToolExecution('create_order', params, result);

// Get context-aware error messages
const message = context.getContextualErrorMessage(error);

// Rollback operations
await context.rollback(operationId);

// Get session summary
const summary = context.getSummary();
```

**Integration Points:**
- MCP tools automatically call `recordToolExecution()` before and after execution
- Schema validator uses `getContextualErrorMessage()` for AI-friendly errors
- Tool composer uses `rollback()` for atomic orchestrations
- Session persistence uses `getSummary()` for state saving

---

### 2. Better Transactions (Saga Pattern)
**File:** `/home/dom/stateset-icommerce/crates/stateset-db/src/saga.rs`

**Purpose:** Implement saga pattern for distributed transactions with compensating actions

**Key Features:**
- Step execution with automatic rollback on failure
- Idempotency support (reuses existing idempotency tables)
- Timeout handling for long-running operations
- Event emission for real-time tracking
- Create order with inventory (atomic)
- Process return with restock (atomic)
- Sync with external system (atomic)
- Payment capture workflow (atomic)

**API:**
```rust
// Create saga coordinator
let coordinator = SagaCoordinator::new(db.clone());

// Execute saga with compensating actions
let result = coordinator.execute(CreateOrderWithInventory::new(params)).await?;

// Check saga status
let status = coordinator.get_status(&saga_id).await?;

// Cancel running saga
coordinator.cancel(&saga_id).await?;
```

**Database Schema:**
- `saga_transactions` - Track saga state (pending, active, completed, cancelled, failed)
- `saga_steps` - Individual step states
- Links to existing `idempotency_keys` table for idempotency

**Integration Points:**
- Exposed to MCP layer via FFI bindings
- Used by `mcp-tool-composer.js` for orchestration
- Emits events for observability system
- Automatically handles idempotency for retry-safe operations

---

### 3. Session Persistence
**File:** `/home/dom/stateset-icommerce/cli/src/session-persistence.js`

**Purpose:** Save and load conversation context between sessions

**Key Features:**
- Save/load session state to disk
- Resume interrupted sessions
- Session metadata (agent type, start time, duration)
- Session history / operations
- Session statistics
- Multi-session management

**API:**
```javascript
// Create session manager
const manager = new SessionPersistence('/path/to/sessions');

// Save session
await manager.saveSession(sessionId, context.getSummary());

// Load session
const state = await manager.loadSession(sessionId);

// List sessions
const sessions = await manager.listSessions();

// Delete session
await manager.deleteSession(sessionId);

// Get session stats
const stats = await manager.getSessionStats(sessionId);
```

**Storage Format:**
```json
{
  "sessionId": "sess-20250125-abc123",
  "startedAt": "2025-01-25T10:30:00Z",
  "agentType": "claude",
  "operations": [
    {
      "tool": "create_order",
      "params": { ... },
      "result": { ... },
      "timestamp": "2025-01-25T10:30:01Z",
      "duration": 150,
      "success": true
    }
  ],
  "stats": {
    "totalOperations": 5,
    "successfulOperations": 4,
    "failedOperations": 1
  }
}
```

**Integration Points:**
- Used by MCP server to maintain state across conversations
- Integrates with `mcp-conversation-context.js` for state capture
- Exposed to agents via MCP tools

---

### 4. Expose Saga to MCP Layer
**File:** `/home/dom/stateset-icommerce/crates/stateset-db/src/expose_saga.js`

**Purpose:** FFI bindings to expose saga coordinator to JavaScript/MCP layer

**API:**
```javascript
import { exposeSagaToNode, createSaga, executeSaga, getSagaStatus, cancelSaga } from '@stateset/db/expose_saga';

// Create saga instance
const saga = createSaga(db);

// Execute saga
const result = await executeSaga(saga, 'create_order_with_inventory', params);

// Get status
const status = await getSagaStatus(saga, sagaId);
```

---

## Architecture Integration

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                          AI Agent (Claude/ChatGPT)              │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                   MCP Server (87 tools)                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │       Session Persistence (save/load state)             │   │
│  └────────────────────┬────────────────────────────────────┘   │
│                       │                                            │
│  ┌────────────────────▼────────────────────────────────────┐   │
│  │      Conversation Context (track operations)             │   │
│  │  - recordToolExecution()                                  │   │
│  │  - getSummary() │                                         │   │
│  │  - rollback()    │                                         │   │
│  └────────────────────┬────────────────────────────────────┘   │
│                       │                                            │
│  ┌────────────────────▼────────────────────────────────────┐   │
│  │      Tool Composer (orchestrate workflows)              │   │
│  │  - orchestrate() → uses saga for atomicity              │   │
│  └────────────────────┬────────────────────────────────────┘   │
│                       │                                            │
│  ┌────────────────────▼────────────────────────────────────┐   │
│  │      Schema Validator (AI-friendly errors)              │   │
│  │  - uses context for contextual error messages          │   │
│  └────────────────────┬────────────────────────────────────┘   │
└───────────────────────┼───────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│              Saga Coordinator (Rust, via FFI)                   │
│  - execute()                                                    │
│  - get_status()                                                 │
│  - cancel()                                                     │
└─────────────────────┬───────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│              StateSet Database (SQLite/PostgreSQL)              │
│  - saga_transactions table                                      │
│  - saga_steps table                                             │
│  - idempotency_keys table                                       │
│  - All domain tables (orders, inventory, etc.)                 │
└─────────────────────────────────────────────────────────────────┘
```

### Event Flow

```
Tool Called → Context.recordExecution()
             → Execute Tool (via Repository)
             → Context.recordResult()
             → Emit Event
             → Observer.updateSummary()
```

### Rollback Flow (Saga)

```
Saga.execute() → Execute Step 1
                → Validate
                → Execute Step 2
                → Validate
                → Execute Step 3
                → [FAILS]
                → Trigger Rollback
                → Compensate Step 2
                → Compensate Step 1
                → Mark saga as failed
                → Emit error event
```

---

## Usage Examples

### Example 1: Create Order with Context Tracking

```javascript
import { ConversationContext } from './mcp-conversation-context.js';
import { SessionPersistence } from './session-persistence.js';
import { createSaga, executeSaga } from '@stateset/db/expose_saga.js';

// Create context
const context = new ConversationContext(commerce, 'sess-123');

// Execute order creation with saga for atomicity
const result = await executeSaga(saga, 'create_order_with_inventory', {
  customerId: 'cust-123',
  items: [{ sku: 'PROD-001', quantity: 2 }]
});

// Record in context
context.recordToolExecution('create_order_with_inventory', params, result);

// Save session
await sessionManager.saveSession('sess-123', context.getSummary());
```

### Example 2: Resume Interrupted Session

```javascript
// Load previous session
const state = await sessionManager.loadSession('sess-123');

// Create context from saved state
const context = ConversationContext.fromState(commerce, state);

// Check what was being done
const lastOperation = context.getLastOperation();
console.log('Last operation:', lastOperation);

// Continue where we left off
if (lastOperation.tool === 'reserve_inventory' && lastOperation.success) {
  // Continue with order creation
  await context.recordToolExecution('create_order', { ... });
}
```

### Example 3: Complex Orchestration with Rollback

```javascript
import { ToolComposer } from './mcp-tool-composer.js';

const composer = new ToolComposer(commerce, context);

const result = await composer.orchestrate('complete-checkout', [
  {
    tool: 'reserve_inventory',
    params: { sku: 'PROD-001', quantity: 2 },
    rollback: async (result) => {
      return await commerce.inventory.releaseReservation({ reservationId: result.id });
    }
  },
  {
    tool: 'create_order',
    params: { customerId: 'cust-123', items: [...] },
    rollback: async (result) => {
      return await commerce.orders.cancel({ orderId: result.id });
    }
  },
  {
    tool: 'process_payment',
    params: { orderId: 'ord-123', amount: 99.99 },
    rollback: async (result) => {
      return await commerce.payments.refund({ paymentId: result.id });
    }
  }
]);

// If any step fails, previous steps are automatically rolled back
```

---

## Testing Strategy

### Unit Tests
- `tests/conversation_context_test.js` - Test context tracking and rollback
- `tests/session_persistence_test.js` - Test save/load functionality
- `tests/saga_test.rs` - Test saga execution and rollback

### Integration Tests
- Scenario: Create order with inventory reservation (atomic)
- Scenario: Process return with restock (atomic)
- Scenario: Resume interrupted session
- Scenario: Rollback failed orchestration

### Performance Tests
- Measure overhead of context tracking (< 5ms per operation)
- Measure saga execution overhead (< 10ms per step)
- Test session save/load performance (< 50ms)

---

## Breaking Changes

None. All new files are additive and do not modify existing APIs.

---

## Dependencies

### JavaScript
- `events` (built-in) - EventEmitter for context and composer
- `fs`, `path` (built-in) - Session persistence

### Rust
- `sqlx` - Async database queries (already a dependency)
- `tokio` - Async runtime (already a dependency)
- `uuid` - UUID generation (already a dependency)
- `serde_json` - JSON serialization (already a dependency)

---

## Future Enhancements

### Phase 4 (Potential)
1. **Rate Limiting** - Prevent runaway agent loops
2. **Multi-Tenancy** - Isolate agent contexts
3. **Session Templates** - Pre-defined conversation patterns
4. **Audit Trail** - Persistent log of all operations
5. **Session Sharing** - Allow collaboration between agents

### Performance Optimizations
- Batch writes for session persistence
- Compress old sessions
- Cache hot session data

### Security Enhancements
- Session encryption
- Role-based access control for sessions
- Audit logs for sensitive operations

---

## Metrics & Observability

### Key Metrics to Track
- Session duration
- Operation success rate
- Rollback frequency
- Saga execution time
- Session storage size

### Example Metrics
```rust
metrics::counter!("saga.execution.total", 1, "type" => "create_order_with_inventory");
metrics::histogram!("saga.execution.duration", duration.as_millis());
metrics::counter!("saga.rollback.total", 1, "reason" => "step_failed");
metrics::counter!("session.operations.total", 1, "session_id" => sessionId);
```

---

## Migration Guide

### For Existing MCP Tools

**Before:**
```javascript
async function createOrder(params) {
  return await commerce.orders.create(params);
}
```

**After:**
```javascript
async function createOrder(context, params) {
  // Record start
  context.recordToolExecution('create_order', params);

  try {
    const result = await commerce.orders.create(params);
    
    // Record success
    context.completeOperation('create_order', result);
    
    return result;
  } catch (error) {
    // Record failure
    context.recordError('create_order', error);
    
    // Get contextual error message
    const message = context.getContextualErrorMessage(error);
    throw new Error(message);
  }
}
```

---

## Documentation Updates

### CLI Manual
- Added `session-persistence` section
- Updated `mcp-server` section with context tracking
- Added `saga-coordinator` section

### Rust API Docs
- Added `saga` module documentation
- Added FFI binding docs for JavaScript

### TypeScript Definitions
- Added type definitions for `ConversationContext`
- Added type definitions for `SessionPersistence`
- Added type definitions for saga FFI

---

## Conclusion

Phase 3 implements the three critical gaps for production deployment:

1. **Conversation Context** - Agents now have memory of previous operations
2. **Better Transactions** - Multi-step operations are atomic with rollback
3. **Session Persistence** - Agent state can be saved and resumed

These systems work together to provide:
- **Reliability** - Rollback prevents partial state corruption
- **Observability** - Track what agents are doing
- **Continuity** - Resume interrupted conversations
- **Safety** - Context-aware error messages guide agents back to valid state

**Next Steps:**
1. Add unit and integration tests
2. Write migration guide for existing MCP tools
3. Create example agent workflows
4. Add telemetry and metrics
5. Consider Phase 4 enhancements (rate limiting, multi-tenancy)