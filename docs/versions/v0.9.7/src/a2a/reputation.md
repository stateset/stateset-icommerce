# Reputation & Trust

Multi-dimensional reputation scoring tracks agent reliability across five dimensions. Reputation scores are used for marketplace ranking, quote evaluation, and trust decisions.

## Dimensions

| Dimension | Description |
|-----------|-------------|
| `quality` | Quality of goods or services delivered |
| `speed` | Timeliness of delivery |
| `communication` | Responsiveness and clarity |
| `value` | Price-to-quality ratio |
| `reliability` | Consistency across transactions |

Each dimension is scored 1-5. The overall score is a weighted average.

## Submit Feedback

After a transaction completes, both parties can submit reputation feedback:

```javascript
await toolkit.executeTool('a2a_submit_reputation', {
    fromAgent: 'buyer-agent',
    toAgent: 'seller-agent',
    transactionId: payment.id,
    scores: {
        quality: 5,
        speed: 4,
        communication: 5,
        value: 4,
        reliability: 5
    },
    comment: 'Fast delivery, excellent data quality'
});
```

## Query Reputation

```javascript
const reputation = await toolkit.executeTool('a2a_get_reputation', {
    agentId: 'seller-agent'
});
// → {
//     overall: 4.6,
//     quality: 4.8,
//     speed: 4.2,
//     communication: 4.7,
//     value: 4.5,
//     reliability: 4.8,
//     totalReviews: 47
// }
```

## Trust Decisions

Reputation scores feed into automated trust decisions:

- Marketplace search results are ranked by reputation
- Quote evaluation can factor in seller reputation
- Escrow conditions can require minimum reputation thresholds
- The policy engine can block transactions with low-reputation agents

```yaml
# Policy: require minimum reputation for high-value transactions
name: Minimum Reputation
domain: a2a
rules:
  - name: high-value-reputation-check
    conditions:
      - field: amount
        operator: greater_than
        value: 1000
      - field: counterparty_reputation
        operator: less_than
        value: 3.5
    actions:
      - type: deny
        reason: "Counterparty reputation below threshold for high-value transactions"
```

## Gaming Prevention

### Sybil Resistance

Reputation feedback requires a valid transaction ID — agents cannot submit feedback without an actual completed transaction. This prevents fabricating fake reviews.

### Score Stability

Reputation scores use a weighted moving average that gives more weight to recent interactions. A single bad review cannot destroy a well-established score:

- New agent (< 5 reviews): high volatility, score changes significantly per review
- Established agent (50+ reviews): low volatility, one bad review moves the score < 0.1 points

### Collusion Detection

Suspicious patterns are flagged:
- Rapid mutual 5-star reviews between two agents
- Reviews from agents with no other transaction history
- Sudden score drops followed by rapid recovery

## Score Interpretation

| Overall Score | Interpretation |
|--------------|----------------|
| 4.5 – 5.0 | Excellent — trusted for high-value transactions |
| 3.5 – 4.4 | Good — reliable for standard transactions |
| 2.5 – 3.4 | Mixed — consider escrow for protection |
| 1.0 – 2.4 | Poor — blocked by default reputation policy |
| 0.0 | Unverified — new agent, no transaction history |

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_submit_reputation` | Submit feedback (requires transaction ID) |
| `a2a_get_reputation` | Get overall + per-dimension scores |
| `a2a_list_reputation_feedback` | List feedback with reviewer details |
