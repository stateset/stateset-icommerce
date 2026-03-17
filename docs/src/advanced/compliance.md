# Compliance & Audit

iCommerce provides built-in tools for regulatory compliance, audit trails, data export, and GDPR data handling.

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
    startDate: '2026-03-01',
    endDate: '2026-03-16'
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
    startDate: '2026-03-01',
    endDate: '2026-03-16'
});

// CSV export
await toolkit.executeTool('audit_export', {
    format: 'csv',
    tool: 'a2a_pay'     // Filter to payment operations
});
```

### Retention Policy

```javascript
// Set retention: archive after 90 days, delete after 365
await toolkit.executeTool('audit_retention_policy', {
    archiveAfterDays: 90,
    deleteAfterDays: 365
});
```

## Compliance Reports

### Tax Report

Multi-jurisdiction tax report for a period:

```javascript
await toolkit.executeTool('export_tax_report', {
    startDate: '2026-01-01',
    endDate: '2026-03-31',
    format: 'csv'
});
```

### SOC 2 Evidence Package

Generate a compliance evidence package:

```javascript
await toolkit.executeTool('export_soc2_evidence', {
    period: '2026-Q1',
    format: 'json'
});
// → { accessControls, auditLogs, changeManagement, incidentResponse }
```

### Compliance Certification

Generate a signed compliance report:

```javascript
await toolkit.executeTool('audit_compliance_cert', {
    period: '2026-03',
    standard: 'SOC2'
});
```

## GDPR Data Handling

### Data Export (Right to Portability)

Export all data for an agent or customer:

```javascript
// Export all transactions for an agent
await toolkit.executeTool('export_gdpr_subject_data', {
    agentAddress: 'agent-xyz',
    format: 'json'
});
// → { payments, quotes, escrows, subscriptions, reputation, messages }
```

### Data Erasure (Right to Erasure)

Delete all data for an agent:

```javascript
await toolkit.executeTool('request_gdpr_erasure', {
    agentAddress: 'agent-xyz',
    confirm: true
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
// Generate a receipt for a specific event
const receipt = await toolkit.executeTool('generate_receipt_bundle', {
    eventId: 'evt-abc123'
});
// → { event, signature, merkleProof, timestamp, publicKey }

// Verify a receipt
const valid = await toolkit.executeTool('verify_receipt', {
    receipt: receipt
});
// → { valid: true, signer: 'ed25519:abc...', timestamp: '...' }

// Generate an inclusion proof (event was in a specific batch)
const proof = await toolkit.executeTool('generate_inclusion_proof', {
    eventId: 'evt-abc123',
    batchId: 'batch-456'
});

// Generate a full compliance package
const pkg = await toolkit.executeTool('generate_compliance_package', {
    startDate: '2026-03-01',
    endDate: '2026-03-16'
});
// → { events, receipts, merkleRoots, signatures, metadata }
```

## Circuit Breaker / Kill Switch

Emergency safety controls for production:

```javascript
// Check agent circuit breaker state
await toolkit.executeTool('agent_get_breaker_state', { agentId: 'my-agent' });
// → { state: 'closed', trippedAt: null, reason: null }

// Set spending limits
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'my-agent',
    dailyLimit: 100.00,
    monthlyLimit: 2000.00,
    perTransactionLimit: 50.00
});

// View spending summary
await toolkit.executeTool('agent_get_spending_summary', { agentId: 'my-agent' });
// → { dailySpend: 45.00, monthlySpend: 890.00, limits: { daily: 100, monthly: 2000 } }

// Manual trip (halt all agent transactions)
await toolkit.executeTool('agent_trip_breaker', {
    agentId: 'my-agent',
    reason: 'Suspicious activity detected'
});

// Global kill switch (halt ALL agents)
await toolkit.executeTool('circuit_breaker_kill_switch', {
    reason: 'Emergency: sequencer compromise suspected',
    confirm: true
});

// Reset after investigation
await toolkit.executeTool('agent_reset_breaker', { agentId: 'my-agent' });
```

## MCP Tools

| Tool | Category | Description |
|------|----------|-------------|
| `audit_query` | Audit | Query audit log entries |
| `audit_summary` | Audit | Aggregate statistics |
| `audit_export` | Audit | JSON/CSV export |
| `audit_retention_policy` | Audit | Set archival rules |
| `audit_compliance_cert` | Audit | Generate compliance report |
| `export_tax_report` | Tax | Multi-jurisdiction tax report |
| `export_gdpr_subject_data` | GDPR | Data portability export |
| `request_gdpr_erasure` | GDPR | Data deletion |
| `export_soc2_evidence` | Compliance | SOC 2 evidence package |
| `verify_receipt` | VES | Verify receipt signature |
| `generate_inclusion_proof` | VES | Merkle inclusion proof |
| `generate_receipt_bundle` | VES | Complete receipt package |
| `generate_compliance_package` | VES | Full audit package |
| `agent_get_breaker_state` | Safety | Check circuit breaker |
| `agent_set_spending_limits` | Safety | Set spending caps |
| `agent_trip_breaker` | Safety | Halt an agent |
| `circuit_breaker_kill_switch` | Safety | Emergency stop all |
