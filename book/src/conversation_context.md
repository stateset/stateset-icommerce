# Conversation Context Management

The Conversation Context module tracks and manages all interactions within an AI agent's session, providing:

- **Full operation timeline**: Track every tool call with timestamps
- **Stateful context**: Maintain conversation state across multiple tool invocations
- **Atomic rollback**: Enable rolling back failed operations
- **Context-aware error messages**: Provide explanations based on conversation history
- **Next action suggestions**: Recommend operations based on current state

## Key Features

### Timeline Tracking
Every tool call is recorded with:
- Timestamp
- Tool name and parameters
- Result or error details
- Duration
- Related entities (order IDs, customer IDs, etc.)

### Rollback Support
Failed operations can be rolled back by:
- Tracking compensating actions for each tool
- Executing compensations in reverse order
- Preserving partial execution state for recovery

### Context-Aware Errors
When an error occurs, the system provides:
- What went wrong
- Why it happened (based on conversation history)
- How to fix it
- Recommended next steps

### Next Action Suggestions
Based on current state and conversation history:
- Suggest likely next operations
- Check prerequisites before suggesting
- Validate if operation makes sense in context

## Usage

```javascript
import { ConversationContext } from './mcp-conversation-context.js';

const context = new ConversationContext(commerce);

// Start a new session
await context.startSession('order-123', { customerId: 'cust-456' });

// Track a tool call
const result = await context.trackToolCall('create_order', {
  customerId: 'cust-456',
  items: [{ sku: 'product-001', quantity: 2 }]
});

// Get context-aware suggestions
const suggestions = await context.getNextSuggestions();
// Returns: [
//   { tool: 'process_payment', reason: 'Order created but not paid', confidence: 0.9 },
//   { tool: 'reserve_inventory', reason: 'Inventory not yet reserved', confidence: 0.85 }
// ]

// Rollback on error
try {
  await context.trackToolCall('process_payment', { ... });
} catch (error) {
  await context.rollback();
}
```

## Integration with MCP Server

The Conversation Context integrates deeply with the StateSet MCP Server:

1. **Automatic tracking**: All MCP tool calls are automatically tracked
2. **Context enhancement**: Tool results are enriched with conversation context
3. **Safety checks**: Warn before potentially destructive operations
4. **Session management**: Manage long-lived agent sessions

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  ConversationContextManager              │
│  - sessions (Map<string, ConversationSession>)          │
│  - maxOperations (default: 100)                         │
└─────────────────────────────────────────────────────────┘
                    │
                    │ manages
                    ▼
┌─────────────────────────────────────────────────────────┐
│                  ConversationSession                     │
│  - id (string)                                          │
│  - sessionId (string)                                   │
│  - startedAt (Date)                                     │
│  - endedAt (Date?)                                      │
│  - operations (Operation[])                             │
│  - state (Map<string, any>)                             │
│  - rollbackStack (RollbackAction[])                     │
└─────────────────────────────────────────────────────────┘
                    │
                    │ contains
                    ▼
┌─────────────────────────────────────────────────────────┐
│                       Operation                         │
│  - id (string)                                          │
│  - tool (string)                                        │
│  - params (any)                                         │
│  - result (any?)                                        │
│  - error (any?)                                         │
│  - timestamp (Date)                                     │
│  - duration (number)                                    │
│  -.rollbackFn (() => Promise<any>)                      │
└─────────────────────────────────────────────────────────┘
```

## Best Practices

1. **Always track tool calls**: Use `trackToolCall` for all commerce operations
2. **Check suggestions before acting**: Use `getNextSuggestions` to understand context
3. **Handle rollbacks appropriately**: Rollback on critical errors, not on all errors
4. **Use context in errors**: Provide contextual error messages to agents
5. **Clean up sessions**: End sessions when complete to free memory

## Session Persistence

Conversation sessions can be persisted to disk for recovery:

```javascript
// Save session to disk
await context.saveSession('/path/to/session.json');

// Load session from disk
await context.loadSession('/path/to/session.json');

// Resume session after interruption
await context.resumeSession('session-id');
```

## Examples

### Creating an Order with Context Tracking

```javascript
const context = new ConversationContext(commerce);
await context.startSession('checkout-session', { customerId: 'cust-123' });

// Step 1: Get cart
const cart = await context.trackToolCall('get_cart', { cartId: 'cart-456' });

// Step 2: Reserve inventory
const reservation = await context.trackToolCall('reserve_inventory', {
  sku: cart.items[0].sku,
  quantity: cart.items[0].quantity,
  referenceType: 'order',
  referenceId: 'pending-order'
});

// Step 3: Create order
const order = await context.trackToolCall('create_order', {
  customerId: context.state.customerId,
  items: cart.items
});

// Validate state
const validation = await context.validateState();
if (!validation.valid) {
  await context.rollback();
  throw new Error('Order creation failed: ' + validation.errors.join(', '));
}
```

### Context-Aware Error Recovery

```javascript
try {
  await context.trackToolCall('update_order_status', {
    orderId: 'order-123',
    status: 'shipped'
  });
} catch (error) {
  // Get contextual error explanation
  const explanation = await context.explainError(error);
  console.log(explanation.message); // "Cannot ship order - payment not processed"
  console.log(explanation.cause);    // "Order status is 'confirmed', payment status is 'pending'"
  
  // Get next action suggestions
  const suggestions = await context.getNextSuggestions();
  console.log(suggestions); // [{ tool: 'process_payment', reason: 'Process payment first' }]
  
  // Auto-recover if possible
  const recovery = await context.attemptAutoRecovery(error);
  if (recovery.canAutoRecover) {
    await recovery.recoveryAction();
  }
}
```

## Performance Considerations

- **Memory**: Each operation in a session consumes memory. Limit to 100 operations by default
- **Disk I/O**: Session persistence reads/writes JSON files. Use asynchronously
- **Context matching**: Next action suggestions use pattern matching - O(n) where n is operations in session

## Future Enhancements

- **ML-based suggestions**: Use machine learning to predict next actions
- **Distributed sessions**: Share context across multiple agent instances
- **Session clustering**: Group similar sessions for pattern discovery
- **Real-time collaboration**: Multiple agents working on same session