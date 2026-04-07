# ADR-0004: CLI Safety Model (`--apply`)

- Status: Accepted
- Date: 2026-02-05

## Context

The CLI accepts natural language instructions from both humans and AI agents and can perform state-changing operations (create, update, delete). Without safeguards, it is easy to run destructive actions by accident — especially when an LLM is making tool calls at machine speed.

We considered three safety models:

1. **No safety** — Every command executes immediately. Fast but dangerous for autonomous agents.
2. **Confirmation prompt** — Ask "are you sure?" before writes. Works for humans, useless for automated agents (they always confirm).
3. **Explicit opt-in** — All writes are blocked by default. A flag (`--apply`) must be explicitly provided to enable mutations. Without it, every command returns a preview of what would happen.

## Decision

All write operations require an explicit `--apply` flag. The default mode is read-only preview.

### How It Works

```bash
# Without --apply: returns a preview
stateset "ship order #12345"
# → "This would change order #12345 status from 'processing' to 'shipped'
#    and create a shipment record. Use --apply to execute."

# With --apply: executes the operation
stateset --apply "ship order #12345 with tracking FEDEX123"
# → "Order #12345 shipped with tracking FEDEX123"
```

### For AI Agents

```javascript
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    allowApply: false   // Agent can only preview
});

// Preview a mutation
const preview = await toolkit.simulateMutation({
    tool: 'ship_order',
    params: { orderId: '12345' }
});
// → { wouldAffect: { order: { from: 'processing', to: 'shipped' } } }

// The LLM can inspect the preview and decide whether to proceed
```

### High-Risk Tool Approval

Beyond `--apply`, 20 high-risk tools require explicit approval even when apply mode is enabled:

- `delete_customer`, `cancel_order`, `refund_payment`
- All A2A payment tools (`a2a_pay`, `a2a_fund_escrow`, etc.)
- Policy modification tools (`reload_policies`, etc.)

These are configured in `permissions.js` with `requireApprovalFor`.

## Consequences

**Positive:**
- Safer day-to-day usage — no accidental mutations from typos or misunderstood instructions
- Clear separation between exploration (read) and execution (write)
- AI agents can safely explore commerce state without risk of unintended changes
- LLMs can reason about previews before committing — the reasoning loop is: intent → preview → decide → apply
- Audit trail clearly shows which operations were previewed vs. executed

**Negative:**
- Automation must explicitly opt in to writes, which adds a small amount of friction
- Two-step execution (preview then apply) is slower than direct execution
- Users who want "just do it" behavior must remember the `--apply` flag

## Alternatives Considered

**Dry-run + confirm workflow**: Show preview, then ask "Execute? [y/N]". Rejected because it requires interactive input, which breaks headless automation and MCP tool calls.

**Role-based access**: Different API keys for read-only vs read-write access. Implemented separately in the [HTTP gateway permissions](../guides/permissions.md) but not sufficient for CLI safety since the CLI runs locally.
