# Event Streaming

Server-Sent Events (SSE) push real-time A2A events to connected agents. Supports wildcard and prefix event matching, persistent event log, and automatic reconnection.

## Connect to the Event Stream

```javascript
const eventSource = new EventSource('/a2a/events?filter=payment.*');

eventSource.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log(`Event: ${data.type}`, data.payload);
};
```

## Event Filtering

| Pattern | Matches |
|---------|---------|
| `payment.*` | `payment.created`, `payment.settled`, etc. |
| `escrow.*` | `escrow.funded`, `escrow.released`, etc. |
| `quote.*` | `quote.requested`, `quote.accepted`, etc. |
| `subscription.*` | `subscription.created`, `subscription.charged`, etc. |
| `*` | All events |

## Event Types

| Event | Description |
|-------|-------------|
| `payment.created` | New payment initiated |
| `payment.settled` | Payment settled on-chain |
| `escrow.funded` | Escrow received funds |
| `escrow.released` | Escrow released to payee |
| `escrow.expired` | Escrow timed out |
| `quote.requested` | New quote request |
| `quote.accepted` | Quote accepted |
| `quote.rejected` | Quote rejected |
| `subscription.created` | New subscription |
| `subscription.charged` | Recurring charge |
| `subscription.cancelled` | Subscription ended |
| `dispute.opened` | Dispute filed |
| `dispute.resolved` | Dispute resolved |
| `reputation.submitted` | New feedback |

## Persistent Event Log

All events are stored in `a2a_event_log` for replay and audit. The log supports `Last-Event-ID` for reconnection — if a client disconnects, it can resume from the last received event ID without missing any events.

## Heartbeat

The SSE connection sends a heartbeat every 30 seconds to keep the connection alive through proxies and load balancers.

## Subscribe to Events (Webhook)

For non-SSE clients, subscribe to events via webhooks:

```javascript
await toolkit.executeTool('a2a_subscribe_events', {
    eventPattern: 'payment.*',
    webhookUrl: 'https://my-agent.example.com/webhooks/a2a',
    secret: 'whsec_...'
});
```

Webhook payloads are signed with HMAC-SHA256 for verification.

## Reconnection with Last-Event-ID

If a client disconnects, it can resume from the last received event without missing anything:

```javascript
// Browser EventSource handles this automatically
const eventSource = new EventSource('/a2a/events?filter=payment.*');
// On reconnect, the browser sends Last-Event-ID header → server resumes from that point

// Manual reconnection with custom logic
const lastId = localStorage.getItem('lastEventId');
const url = lastId
    ? `/a2a/events?filter=payment.*&lastEventId=${lastId}`
    : '/a2a/events?filter=payment.*';
```

## Backpressure

If a client cannot keep up with the event rate:

1. Events are buffered server-side (bounded queue)
2. If the buffer fills, oldest events are dropped
3. The client receives a `backpressure` event indicating missed events
4. The client should replay from the event log to catch up

## Event Payload Shape

```json
{
    "id": "evt-abc123",
    "type": "payment.settled",
    "timestamp": "2026-03-16T10:30:45Z",
    "payload": {
        "paymentId": "pay-xyz",
        "amount": 450.00,
        "currency": "USD",
        "payer": "buyer-agent",
        "payee": "seller-agent",
        "txHash": "0x..."
    }
}
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_subscribe_events` | Subscribe to events (SSE or webhook) |
| `a2a_unsubscribe_events` | Remove a subscription |
| `a2a_list_event_subscriptions` | List active subscriptions |
| `a2a_replay_events` | Replay events from a point in time |
| `a2a_get_event_log` | Query the persistent event log (filter, paginate) |
