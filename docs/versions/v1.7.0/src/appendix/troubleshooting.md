# Troubleshooting

Common issues and their solutions.

## Installation

### `npm install` fails in `cli/` directory

The `@stateset/embedded` package can't resolve from within the `cli/` directory. Install at the repository root instead:

```bash
cd /path/to/stateset-icommerce
npm install
```

### `stateset-init` command not found

Ensure the CLI is installed globally:

```bash
npm install -g @stateset/cli
```

## Database

### "Database is locked"

SQLite allows only one writer at a time. If you see lock errors:

1. Ensure only one process is writing to the database
2. Check for lingering processes: `lsof store.db`
3. Enable WAL mode for better concurrent read performance

### Migration Errors

If the schema is out of date:

```bash
stateset-init --quickstart  # Re-initializes with latest schema
```

## CLI

### "Permission denied" for write operations

All write operations require the `--apply` flag:

```bash
# This will fail (read-only by default)
stateset "create a customer named Alice"

# This works
stateset --apply "create a customer named Alice"
```

### "Tool not found" errors

Ensure you're using the correct version:

```bash
npm install -g @stateset/cli@latest
```

### Vector search returns no results

Hybrid search requires an OpenAI API key:

```bash
export OPENAI_API_KEY=sk-...
stateset "find products similar to wireless earbuds"
```

Without the key, search falls back to keyword matching only.

## Adapters

### Stripe webhook signature verification fails

1. Ensure the webhook secret matches: `whsec_...`
2. Check that the raw body is not parsed before verification
3. Verify the clock difference is less than 5 minutes

### WooCommerce connection refused

1. Verify the store URL uses HTTPS (required for Basic Auth)
2. Check that REST API keys have Read/Write permissions
3. Ensure the store URL doesn't end with a trailing slash

### Shopify CSV import errors

1. Use the standard Shopify CSV export format
2. Ensure CSV files are UTF-8 encoded
3. Check for missing required columns (SKU, name, price)

## A2A Protocol

### Budget exceeded errors

```json
{ "error": "BudgetExceededError", "remaining": 0.20, "attempted": 0.50 }
```

Increase the daily budget:

```javascript
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'my-agent',
    dailyLimit: 10.00,
    monthlyLimit: 100.00,
    perTransactionLimit: 1.00
});
```

### Circuit breaker is open

The x402 client circuit breaker opens when the sequencer is unreachable.

Wait for the circuit breaker timeout, or check sequencer connectivity.

## Tests

### CLI tests hang

Event stream tests create a heartbeat `setInterval`. Ensure cleanup functions are called in `afterEach`:

```javascript
afterEach(() => {
    if (cleanup) cleanup();
});
```

### `scheduler.test.js` is slow

The `cancelJob` test takes ~60 seconds due to executor timeout. This is expected behavior.

### `capture.js` mock causes unhandled rejection

The outbox in `capture.js` triggers async crypto signing. When mocking in tests, ensure you also mock the crypto signing path to prevent unhandled promise rejections.

## Performance

### Slow queries

1. Check that indexes exist for your query patterns (see [Performance Tuning](../guides/performance.md))
2. Use pagination for large result sets
3. Use `list_by_status` or `list_by_customer` instead of `list()` + filter

### High memory usage

1. Reduce the event channel capacity
2. Disable unused features (webhooks, heartbeat)
3. Use `:memory:` database for testing (no disk I/O)

## Error Message Reference

Quick lookup for common error messages and their solutions.

### Commerce Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `NotFound: Order ORD-999 not found` | Invalid order ID | Use `list_orders` to find valid IDs |
| `InvalidState: Cannot ship from Pending` | Wrong status transition | Move to `processing` first: `update_order_status(id, 'processing')` |
| `InvalidState: Cannot cancel shipped order` | Order already shipped | Use `create_return` instead of `cancel_order` |
| `Validation: Email is required` | Missing required field | Include all required fields in the request |
| `Conflict: SKU WIDGET-001 already exists` | Duplicate SKU | Use a different SKU or update the existing product |

### Policy Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `PolicyDeniedError: Return exceeds 30-day window` | Return policy rule | Check `remediation` field — may suggest escalation |
| `PolicyDeniedError: Amount exceeds approval threshold` | Spending policy | Reduce amount or request manager approval |
| `PolicyDeniedError: Final sale items cannot be returned` | Product restriction | Check product tags; contact support for warranty |

### A2A Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `BudgetExceededError: Daily budget exhausted` | Agent spending cap reached | Wait for reset or increase `agent_set_spending_limits` |
| `QuoteExpiredError: Quote expired 2 hours ago` | Past `validUntil` | Request a new quote with `a2a_request_quote` |
| `MaxRoundsExceededError: 5 counter-offers reached` | Negotiation limit | Accept, reject, or start a new quote |
| `EscrowExpiredError: Past expiration date` | No conditions fulfilled in time | Funds auto-refunded to payer |
| `CircuitOpenError: Circuit breaker is open` | Sequencer unreachable | Wait for timeout or check sequencer health |

### Adapter Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `SSRFBlockedError: Webhook URL is a private IP` | SSRF protection triggered | Use a public URL; for local dev use `ngrok` or `stripe listen` |
| `SignatureVerificationError: Invalid signature` | Wrong webhook secret | Verify secret matches the one in your Stripe/WC/Shopify dashboard |
| `SignatureVerificationError: Timestamp too old` | Clock skew > 5 minutes | Sync your server clock with NTP |

### Database Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `Database: database is locked` | Concurrent write attempt (SQLite) | Ensure only one writer; consider PostgreSQL for concurrency |
| `Database: no such table: orders` | Schema not initialized | Run `stateset-init --quickstart` to create tables |
| `Database: UNIQUE constraint failed` | Duplicate insert | Operation already completed; idempotency working correctly |

### Infrastructure Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `ECONNREFUSED: localhost:3000` | Webhook server not running | Start: `stateset-webhooks --port 3000` |
| `ECONNREFUSED: sequencer` | Sequencer unreachable | Check network; circuit breaker will retry automatically |
| `require is not defined` | Using `require()` in ES modules | Switch to `import { Commerce } from '@stateset/embedded'` |
