# The Agentic Reasoning Loop

Understanding how an LLM interacts with iCommerce is critical to understanding the system's design. Every agent operation follows a structured reasoning loop that combines LLM intelligence with deterministic execution.

## The Loop

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         LLM Reasoning Engine                             │
│                    (Claude, GPT, Gemini, Ollama)                         │
└────────┬───────────────────────────────┬────────────────────────────────┘
         │                               │
    1. Natural Language              6. Observe Result
    Intent / Context                 & Reason About
         │                           Next Step
         ▼                               ▲
┌────────────────┐              ┌────────────────┐
│  2. Select     │              │  5. Execute    │
│  MCP Tool      │              │  State Change  │
│  (from 520+)   │              │  (if --apply)  │
└───────┬────────┘              └───────┬────────┘
        │                               ▲
        ▼                               │
┌────────────────┐              ┌────────────────┐
│  3. Preview    │──[allowed]──►│  4. Policy     │
│  (dry run)     │              │  Evaluation    │
│                │              │                │
│  Returns what  │              │  Deny → return │
│  would change  │              │  structured    │
│                │◄─[denied]────│  explanation   │
└────────────────┘              │  with remedy   │
                                └────────────────┘
                                        │
                                        ▼
                                ┌────────────────┐
                                │  7. Sign &     │
                                │  Sync Event    │
                                │  (VES v1.0)    │
                                └────────────────┘
```

## Step by Step

### 1. Natural Language Intent

The user (or another agent) provides a natural language instruction:

> "Ship order #12345 with tracking number FEDEX-789"

The LLM parses this into a structured intent: tool = `ship_order`, params = `{ orderId: "12345", trackingNumber: "FEDEX-789" }`.

### 2. Tool Selection

The LLM selects from the available MCP tools. Each tool has a JSON schema describing its parameters, making selection deterministic once the intent is parsed.

### 3. Preview (Dry Run)

Without `--apply`, the tool returns a preview:

```json
{
  "preview": true,
  "wouldAffect": {
    "order": { "id": "12345", "currentStatus": "processing", "newStatus": "shipped" },
    "shipment": { "trackingNumber": "FEDEX-789", "carrier": "FedEx" },
    "inventory": { "reservationsReleased": 3 }
  }
}
```

The LLM can inspect this preview and confirm it matches the user's intent.

### 4. Policy Evaluation

The policy engine evaluates the operation against all matching rules:

- Is this agent authorized to ship orders?
- Does the order meet shipping criteria (payment captured, address verified)?
- Are there any holds or flags on this order?

If denied, the engine returns a structured explanation the LLM can reason about.

### 5. Execute

With `--apply` and policy approval, the state change is committed atomically to the database.

### 6. Observe & Reason

The LLM receives the execution result and can:
- Confirm success to the user
- Chain to the next step (e.g., send notification)
- Handle errors by reading the structured error and retrying with corrected parameters

### 7. Sign & Sync

The state change is captured as an event, signed with Ed25519, and (if Tier 2+) pushed to the sequencer for multi-agent coordination.

## Why This Matters

### Preview-first prevents catastrophic errors

When an agent calls a tool without `--apply`, it receives a structured preview showing exactly what would change. The LLM can reason about this preview, confirm it matches the user's intent, and only then issue the mutating call. This eliminates the "fire and forget" pattern that makes autonomous agents dangerous.

### Explainable denials prevent retry loops

Traditional APIs return opaque error codes (`400 Bad Request`) that cause LLMs to retry the same failing request in a loop. iCommerce's policy engine returns structured denials with per-condition breakdowns: which field failed, what was expected vs. actual, and a human-readable remediation string. This explanation flows directly into the LLM's context window, enabling the agent to autonomously correct its parameters and retry without human intervention.

### Determinism enables simulation

Because every operation is a pure function of inputs and database state, agents can simulate entire workflows before committing:

```javascript
const plan = await toolkit.executePlan({
  dryRun: true,
  steps: [
    { tool: 'create_order', params: { ... } },
    { tool: 'capture_payment', params: { ... } },
    { tool: 'ship_order', params: { ... } },
  ]
});
// plan.steps[0].preview, plan.steps[1].preview, etc.
```

This is how autonomous agents can safely operate at scale: simulate, verify, then execute.
