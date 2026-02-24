# Autonomous Multi-Agent Examples

This directory contains practical examples of running two or more autonomous agents that coordinate with each other in iCommerce.

## Examples

### 1. Simple Scheduled Agents
**File:** `2-scheduled-agents.js`

Two agents running independently on schedules:
- **Inventory Monitor Agent** - Runs every hour, checks stock levels
- **Fulfillment Agent** - Runs every 15 minutes, processes orders

They interact through shared database state (no direct communication needed).

### 2. State Machine Workflow
**File:** `2-workflow-agents.js`

Two agents hand off work through a state machine:
- **Inventory Agent** - Reserves items for new orders
- **Payments Agent** - Confirms payment and triggers fulfillment

They cross-trigger each other via state transitions.

### 3. Policy-Driven Coordination
**File:** `2-policy-agents.js`

Two agents coordinate through event-driven policies:
- **Low Stock Agent** - Detects when inventory drops below threshold
- **Supplier Agent** - Automatically creates purchase orders when alerted

The second agent is triggered by the first agent's actions.

## Running the Examples

Each example can be run independently:

```bash
# Example 1: Scheduled jobs
node 2-scheduled-agents.js

# Example 2: Workflow handoff
node 2-workflow-agents.js

# Example 3: Policy-driven coordination
node 2-policy-agents.js
```

## Key Patterns

| Pattern | Coordination Method | Use Case |
|---------|-------------------|----------|
| **Scheduled Jobs** | Shared database state | Independent periodic tasks |
| **State Machine** | State transitions | Sequential handoffs between agents |
| **Policy Engine** | Event-driven reactions | Reactive agent coordination |

## Requirements

- Node.js 20.20.0+ and npm 10.0.0+
- iCommerce CLI installed
- PostgreSQL database configured
