# Quotes & Negotiation

The A2A quote protocol enables structured price negotiation between agents with a configurable maximum number of rounds to prevent infinite loops.

## Request a Quote

```javascript
const quote = await toolkit.executeTool('a2a_request_quote', {
    fromAgent: 'agent-buyer-001',
    toAgent: 'agent-seller-002',
    items: [
        { description: 'Market data feed - 30 day license', quantity: 1 }
    ],
    maxPrice: 500.00,
    currency: 'USD',
    expiresIn: '7d'
});
```

## Respond to a Quote

```javascript
const response = await toolkit.executeTool('a2a_respond_to_quote', {
    quoteId: quote.id,
    items: [
        { description: 'Market data feed - 30 day license', quantity: 1, unitPrice: 450.00 }
    ],
    validUntil: '2026-03-23T00:00:00Z'
});
```

## Counter-Offer

Negotiation continues until one party accepts or the maximum round count (default: 5) is reached:

```javascript
const counter = await toolkit.executeTool('a2a_counter_quote', {
    quoteId: quote.id,
    items: [
        { description: 'Market data feed - 30 day license', quantity: 1, unitPrice: 425.00 }
    ],
    message: 'Can we meet at $425 for a 30-day commitment?'
});
```

## Accept a Quote

Accepting a quote creates a payment intent and (optionally) an escrow:

```javascript
const acceptance = await toolkit.executeTool('a2a_accept_quote', {
    quoteId: quote.id
});
// → { paymentIntentId: '...', escrowId: '...' }
```

## Quote States

```
Draft → Sent → Countered (up to 5 rounds) → Accepted
                                            → Rejected
                                            → Expired (auto, after validUntil)
```

## Quote Expiration

Quotes automatically expire after the `validUntil` timestamp. Attempting to counter or accept an expired quote returns a structured error:

```json
{
    "error": "QuoteExpiredError",
    "quoteId": "q-123",
    "validUntil": "2026-03-23T00:00:00Z",
    "message": "Quote expired 2 hours ago. Request a new quote."
}
```

## Round Limits

The default maximum is 5 counter-offer rounds. After 5 rounds, no further counters are accepted — the parties must accept, reject, or let the quote expire. This prevents negotiation deadlocks between persistent agents.

## Multi-Currency Quotes

Quotes can specify different currencies for buyer and seller. Currency conversion uses the rates from the commerce engine:

```javascript
const quote = await toolkit.executeTool('a2a_request_quote', {
    fromAgent: 'eu-buyer',
    toAgent: 'us-seller',
    currency: 'EUR',         // Buyer pays in EUR
    items: [{ description: 'Widget batch', quantity: 100 }],
    maxPrice: 450.00
});
// Seller responds in USD; conversion applied at acceptance
```

## Error Handling

| Error | Cause | Resolution |
|-------|-------|------------|
| `QuoteExpiredError` | Quote past `validUntil` | Request a new quote |
| `QuoteNotFoundError` | Invalid quote ID | Check `a2a_list_quotes` |
| `MaxRoundsExceededError` | 5 counter-offers reached | Accept, reject, or start new |
| `InvalidStateError` | Action on already-accepted/rejected quote | Check quote status first |
| `AgentNotFoundError` | Target agent doesn't exist | Verify agent card |

## Quote History

Retrieve the full negotiation progression:

```javascript
const quote = await toolkit.executeTool('a2a_get_quote', { quoteId: 'q-123' });
// → {
//     id: 'q-123',
//     status: 'accepted',
//     rounds: [
//         { round: 1, action: 'requested', price: null, by: 'buyer' },
//         { round: 2, action: 'quoted', price: 500, by: 'seller' },
//         { round: 3, action: 'countered', price: 400, by: 'buyer' },
//         { round: 4, action: 'countered', price: 450, by: 'seller' },
//         { round: 5, action: 'accepted', price: 450, by: 'buyer' }
//     ]
// }
```

## Strategy Integration

Automate negotiation decisions with [negotiation strategies](advanced.md#negotiation-strategies):

```javascript
const strategy = createNegotiatorStrategy({
    basePrice: 100,
    minAcceptable: 80,
    concessionRate: 0.15
});

// The strategy evaluates each incoming quote automatically
const decision = strategy.evaluateReceivedQuote(incomingQuote, context);
// → { action: 'counter', counterPrice: 91.20 }
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_request_quote` | Request a quote from another agent |
| `a2a_respond_to_quote` | Respond with pricing |
| `a2a_counter_quote` | Counter-offer (max 5 rounds) |
| `a2a_accept_quote` | Accept and create payment intent |
| `a2a_reject_quote` | Reject with reason |
| `a2a_list_quotes` | List quotes (filter by status, agent, date) |
| `a2a_get_quote` | Get quote details with full round history |
