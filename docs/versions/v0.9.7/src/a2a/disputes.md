# Disputes & Resolution

When an A2A transaction goes wrong, either party can open a dispute. The dispute protocol supports evidence submission, deadline management, and automatic resolution.

## Open a Dispute

```javascript
const dispute = await toolkit.executeTool('a2a_open_dispute', {
    transactionId: payment.id,
    filedBy: 'buyer-agent',
    reason: 'Service not delivered as specified',
    evidence: [
        { type: 'screenshot', description: 'API returned errors', hash: '...' },
        { type: 'log', description: 'Request/response log', hash: '...' }
    ],
    requestedResolution: 'full_refund'
});
```

## Submit Evidence

Both parties can submit evidence during the dispute window:

```javascript
await toolkit.executeTool('a2a_submit_dispute_evidence', {
    disputeId: dispute.id,
    submittedBy: 'seller-agent',
    evidence: [
        { type: 'delivery_proof', description: 'API access logs showing successful delivery', hash: '...' }
    ]
});
```

Evidence hashes are stored for integrity verification — the actual evidence is referenced by hash, not stored in the protocol.

## Resolution

Disputes can be resolved manually or automatically:

```javascript
// Manual resolution
await toolkit.executeTool('a2a_resolve_dispute', {
    disputeId: dispute.id,
    resolution: 'partial_refund',
    amount: 250.00,
    reason: 'Partial service delivery confirmed'
});
```

### Auto-Resolution

The dispute auto-resolver runs on a schedule and can automatically resolve disputes based on rules:

- If the deadline passes with no response from the accused party, resolve in favor of the filer
- If both parties agree on a resolution, execute it immediately
- If evidence is clear-cut (e.g., delivery confirmed on-chain), auto-resolve

## Dispute States

```
Opened → Evidence Phase → Under Review → Resolved
                                       → Escalated
```

## Dispute Timeline Constants

| Phase | Duration | Description |
|-------|----------|-------------|
| Filing → Evidence Period | 24 hours | Grace period before evidence phase begins |
| Evidence Period | 72 hours | Both parties submit evidence |
| Under Review | 7 days | Arbitration rules are applied |
| Resolution | Immediate | Once rules determine outcome |

## Evidence Integrity

Evidence is referenced by SHA-256 hash, not stored in the protocol. This ensures:

- Evidence cannot be tampered with after submission
- The dispute record proves which evidence was submitted and when
- Large files (screenshots, logs) are stored externally; only the hash is on-chain

```javascript
import { createHash } from 'node:crypto';

// Hash evidence before submission
const hash = createHash('sha256').update(evidenceBuffer).digest('hex');

await toolkit.executeTool('a2a_submit_dispute_evidence', {
    disputeId: dispute.id,
    evidence: [{ type: 'screenshot', description: 'API error response', hash }]
});
```

## Error Handling

| Error | Cause | Resolution |
|-------|-------|------------|
| `DisputeNotFoundError` | Invalid dispute ID | Check `a2a_list_disputes` |
| `EvidenceDeadlinePassed` | Evidence submitted after 72h window | Cannot submit; dispute proceeds to review |
| `DisputeAlreadyResolved` | Action on closed dispute | No further action possible |
| `AmountExceedsThreshold` | Auto-resolution over $1,000 | Escalated for manual review |

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_open_dispute` | Open a new dispute |
| `a2a_submit_dispute_evidence` | Submit evidence with hash |
| `a2a_resolve_dispute` | Resolve with resolution type and amount |
| `a2a_list_disputes` | List disputes (filter by status, agent, date) |
| `a2a_get_dispute` | Get dispute details with evidence and timeline |
| `a2a_dispute_resolver_status` | Auto-resolver metrics (ticks, resolutions, escalations) |

See also: [Autonomous Engine — Dispute Auto-Resolution](../guides/autonomous-engine.md) for the rule-based arbitration system.
