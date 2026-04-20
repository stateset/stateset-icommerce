# Agent Memory & Counterparty Learning

The agent memory system enables agents to learn from past interactions with counterparties. Profiles are computed on-demand from interaction history, enabling pattern detection — late fulfillment, habitual negotiation, declining reliability — and risk-aware decision making.

## Why Agent Memory?

Without memory, every transaction starts from zero. An agent can't distinguish a reliable counterparty it has worked with 50 times from a brand-new unknown entity. Agent memory changes this:

- **Risk scoring**: Automatically flag counterparties with declining success rates
- **Negotiation intelligence**: Track average discount percentages and counter-offer patterns
- **Reliability tracking**: Measure response times, fulfillment rates, and dispute frequency
- **Recommendations**: Get confidence-scored recommendations before transacting

## Interaction Types

The memory system tracks 8 types of interactions:

| Type | Direction | What It Records |
|------|-----------|----------------|
| `quote_received` | Inbound | Counterparty sent us a quote |
| `quote_sent` | Outbound | We sent a quote to counterparty |
| `payment_sent` | Outbound | We paid the counterparty |
| `payment_received` | Inbound | Counterparty paid us |
| `negotiation` | Both | Price negotiation round |
| `dispute` | Both | Dispute filed or received |
| `fulfillment` | Both | Service delivery completed or received |
| `rating` | Both | Reputation rating given or received |

Each interaction records: outcome (`success`, `failure`, `timeout`, `rejected`, `accepted`), amount, response time, and arbitrary metadata.

## Recording Interactions

```javascript
const memory = createAgentMemory();

// Record a successful payment
await memory.recordInteraction({
    agentAddress: '0xBuyer',
    counterpartyAddress: '0xSeller',
    interactionType: 'payment_sent',
    outcome: 'success',
    amount: 100,
    responseTimeMs: 1500,
});

// Record a fulfillment timeout
await memory.recordInteraction({
    agentAddress: '0xBuyer',
    counterpartyAddress: '0xSeller',
    interactionType: 'fulfillment',
    outcome: 'timeout',
    amount: 100,
    responseTimeMs: 45000,
});
```

## Counterparty Profiles

Profiles are computed on-demand from the interaction history:

```javascript
const profile = memory.getCounterpartyProfile('0xBuyer', '0xSeller');
// → {
//     counterparty: '0xSeller',
//     totalInteractions: 23,
//     successRate: 0.87,
//     reliabilityScore: 0.82,
//     riskLevel: 'low',
//     avgResponseTimeMs: 3200,
//     firstSeen: '2026-01-15T...',
//     lastSeen: '2026-03-16T...',
//     negotiation: {
//         avgDiscountPct: 12.5,
//         counterOfferRate: 0.45,
//     },
//     riskAlerts: []
// }
```

### Reliability Score

The reliability score combines two factors:

```
reliabilityScore = (0.70 × successRate) + (0.30 × timelinessRate)
```

- **Success rate**: Fraction of interactions with `success` or `accepted` outcomes
- **Timeliness rate**: Fraction of interactions with response time under 10 seconds

### Risk Levels

| Risk Level | Condition |
|------------|-----------|
| **low** | Failure rate < 10% |
| **medium** | Failure rate 10–20%, or < 3 interactions with > 20% failure |
| **high** | Failure rate > 20% with 3+ interactions |

### Risk Alerts

The system raises alerts when recent performance diverges from historical:

- **Declining reliability**: Recent success rate dropped > 20% below overall average (computed over the last 10 interactions)

## Recommendations

Before transacting, ask for a confidence-scored recommendation:

```javascript
const rec = memory.getRecommendation('0xBuyer', '0xSeller', 'payment_sent');
// → {
//     recommended: true,
//     confidence: 0.85,
//     reason: 'Reliable counterparty with 87% success rate across 23 interactions',
//     riskLevel: 'low',
//     suggestedActions: []
// }
```

For risky counterparties:

```javascript
// → {
//     recommended: false,
//     confidence: 0.72,
//     reason: 'High failure rate (35%) with declining recent performance',
//     riskLevel: 'high',
//     suggestedActions: ['require_escrow', 'reduce_amount']
// }
```

## Aggregate Insights

Get a cross-counterparty view of an agent's interaction landscape:

```javascript
const insights = memory.getAgentInsights('0xBuyer');
// → {
//     totalCounterparties: 12,
//     totalInteractions: 156,
//     overallSuccessRate: 0.91,
//     topCounterparties: [...],     // Ranked by reliability
//     riskAlerts: [                  // Active risk warnings
//         { counterparty: '0xRisky', alert: 'declining_reliability', ... }
//     ],
//     interactionBreakdown: {
//         payment_sent: 45,
//         quote_received: 38,
//         fulfillment: 32,
//         ...
//     }
// }
```

## Top Counterparties

Rank counterparties by composite reliability score:

```javascript
const top = memory.getTopCounterparties('0xBuyer', { limit: 5 });
// → [
//     { address: '0xReliable', score: 0.95, interactions: 42 },
//     { address: '0xGood',     score: 0.88, interactions: 28 },
//     ...
// ]
```

## Integration with Other A2A Features

Agent memory feeds into several other systems:

| System | How Memory Is Used |
|--------|-------------------|
| [Rules Engine](rules-engine.md) | `DISPUTE_RATE_BLACKLIST` template uses failure rate |
| [Negotiation Strategies](advanced.md) | `BestOfN` strategy factors in reliability |
| [Fan-Out](advanced.md) | Target selection weighted by counterparty score |
| [Escrow](escrow.md) | Auto-require escrow for high-risk counterparties |

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_agent_memory_record` | Record an interaction |
| `a2a_agent_memory_profile` | Get counterparty profile |
| `a2a_agent_memory_recommendation` | Get recommendation before transacting |
| `a2a_agent_memory_insights` | Get aggregate agent insights |
| `a2a_agent_memory_top` | Rank counterparties by reliability |
