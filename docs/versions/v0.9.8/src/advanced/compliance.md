# Compliance & Audit

iCommerce provides built-in tools for regulatory compliance, audit trails, data export, and GDPR data handling.

Note: this page includes a mix of shipped patterns and target or tier-specific interfaces. Treat a compliance tool as supported only if it is discoverable in your runtime's tool registry or explicitly documented for your deployment tier.

## Audit Trail

Every tool invocation is logged with timestamp, agent ID, tool name, parameters (with PII redacted), and result status.

### Querying the Audit Log

```javascript
// Query by tool name
const entries = await toolkit.executeTool('audit_query', {
    tool: 'ship_order',
    limit: 50
});

// Query by result
const denials = await toolkit.executeTool('audit_query', {
    result: 'denied',
    since: '2026-03-01T00:00:00Z'
});

// Summary statistics
const summary = await toolkit.executeTool('audit_summary', {});
// → {
//     byTool: { list_orders: 1200, create_order: 450, ship_order: 300, ... },
//     byResult: { allowed: 1800, denied: 45, executed: 750 },
//     byAgent: { 'customer-service': 900, 'fulfillment-agent': 600, ... }
// }
```

### Exporting Audit Data

```javascript
// JSON export
await toolkit.executeTool('audit_export', {
    format: 'json',
    since: '2026-03-01T00:00:00Z'
});

// Cross-system CSV export (payments + breaker events + ledger activity)
await toolkit.executeTool('export_audit_trail', {
    format: 'csv',
    from: '2026-03-01T00:00:00Z',
    to: '2026-03-16T23:59:59Z',
    eventType: 'payment'
});
```

### Retention Cleanup

```javascript
// Purge entries older than the configured retention window
await toolkit.executeTool('audit_retention', {});
```

## Compliance Reports

### Tax Report

Multi-jurisdiction tax report for a period:

```javascript
await toolkit.executeTool('generate_1099k', {
    year: 2026,
    agentAddress: 'agent-xyz'
});
```

### SOC 2 Evidence Package

Generate a compliance evidence package:

```javascript
await toolkit.executeTool('soc2_evidence', {
    controls: ['access_control', 'monitoring']
});
// → { evidence: [...], generatedAt: '...' }
```

### Compliance Summary

Generate an aggregate compliance report:

```javascript
await toolkit.executeTool('compliance_summary', {
    period: 'month',
    agentName: 'agent-xyz'
});
```

## GDPR Data Handling

### Data Export (Right to Portability)

Export all data for an agent or customer:

```javascript
// Export all transactions for an agent
await toolkit.executeTool('export_gdpr_data', {
    customerId: 'agent-xyz'
});
// → { payments, quotes, escrows, subscriptions, reputation, messages }
```

### Data Erasure (Right to Erasure)

Delete all data for an agent:

```javascript
await toolkit.executeTool('delete_gdpr_data', {
    customerId: 'agent-xyz',
    keepTransactions: true
});
// Removes: payments, quotes, escrows, subscriptions, reputation, messages, memory
```

### Data Minimization

The audit trail automatically redacts PII from tool parameters:
- Email addresses → `***@***.com`
- API keys → `sk-...***`
- Credit card numbers → `****-****-****-1234`
- Phone numbers → `+1-***-***-0123`

## VES Proof Generation

Generate cryptographic proofs for compliance:

```javascript
// Generate a receipt bundle from a known event + batch
const receipt = await toolkit.executeTool('generate_receipt_bundle', {
    event: JSON.stringify(event),
    batchEvents: JSON.stringify(batchEvents),
    batchId: 'batch-456'
});
// → { bundle: { event, leafHash, proof, root, ... } }

// Verify a receipt
const valid = await toolkit.executeTool('verify_receipt', {
    receiptBundle: JSON.stringify(receipt.bundle)
});
// → { valid: true, signer: 'ed25519:abc...', timestamp: '...' }

// Generate an inclusion proof (event was in a specific batch)
const proof = await toolkit.executeTool('generate_inclusion_proof', {
    eventId: 'evt-abc123',
    events: JSON.stringify(batchEvents),
    batchId: 'batch-456'
});

// Generate a full compliance package
const pkg = await toolkit.executeTool('export_compliance_package', {
    events: JSON.stringify(batchEvents),
    batchId: 'batch-456'
});
// → { events, receipts, merkleRoots, signatures, metadata }
```

## Circuit Breaker / Kill Switch

Emergency safety controls for production:

```javascript
// Check agent circuit breaker state
await toolkit.executeTool('agent_get_breaker_state', { agentName: 'my-agent' });
// → { state: 'closed', trippedAt: null, reason: null }

// Set global spending limits
await toolkit.executeTool('agent_set_spending_limits', {
    dailySpendLimit: 100.00,
    monthlySpendLimit: 2000.00,
    maxSpendPerTx: 50.00
});

// View spending summary
await toolkit.executeTool('agent_get_spending_summary', { agentName: 'my-agent' });
// → { dailySpend: 45.00, monthlySpend: 890.00, limits: { daily: 100, monthly: 2000 } }

// Manual trip (halt all agent transactions)
await toolkit.executeTool('agent_trip_breaker', {
    agentName: 'my-agent',
    reason: 'Suspicious activity detected'
});

// Global kill switch (halt ALL agents)
await toolkit.executeTool('agent_trip_all_breakers', {
    reason: 'Emergency: sequencer compromise suspected'
});

// Reset after investigation
await toolkit.executeTool('agent_reset_breaker', { agentName: 'my-agent' });
```

## MCP Tools

| Tool | Category | Description |
|------|----------|-------------|
| `audit_query` | Audit | Query audit log entries |
| `audit_summary` | Audit | Aggregate statistics |
| `audit_export` | Audit | JSON/CSV export |
| `audit_retention` | Audit | Run retention cleanup |
| `export_audit_trail` | Audit | Cross-system compliance export |
| `generate_1099k` | Tax | Generate 1099-K report |
| `export_gdpr_data` | GDPR | Data portability export |
| `delete_gdpr_data` | GDPR | Data deletion |
| `soc2_evidence` | Compliance | SOC 2 evidence package |
| `compliance_summary` | Compliance | Aggregate compliance report |
| `verify_receipt` | VES | Verify receipt signature |
| `generate_inclusion_proof` | VES | Merkle inclusion proof |
| `generate_receipt_bundle` | VES | Complete receipt package |
| `export_compliance_package` | VES | Full audit package |
| `agent_get_breaker_state` | Safety | Check circuit breaker |
| `agent_set_spending_limits` | Safety | Set spending caps |
| `agent_trip_breaker` | Safety | Halt an agent |
| `agent_trip_all_breakers` | Safety | Emergency stop all |
