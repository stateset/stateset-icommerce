# Examples

Runnable examples live in the `examples/` directory. Each demonstrates a specific workflow or integration pattern.

## Node.js Examples

Located in `examples/node/`:

| File | Description |
|------|-------------|
| `01_getting_started.js` | Basic Commerce instance setup, customer/product/order creation |
| `02_cart_and_checkout.js` | Cart management, item operations, discount application, checkout flow |
| `03_analytics_and_forecasting.js` | Revenue summaries, demand prediction, inventory health |
| `04_subscriptions.js` | Recurring billing: plan creation, subscribe, pause, resume, cancel, churn analysis |
| `05_promotions.js` | Discounts, coupon codes, campaign management |
| `06_currency.js` | Multi-currency conversion, exchange rate management, 150+ currencies |
| `07_tax.js` | Multi-jurisdiction tax (US states, EU VAT, Canadian GST/PST/HST), nexus detection |
| `08_manufacturing.js` | BOM creation, work orders, quality control, yield tracking |
| `09_full_workflow.js` | Complete order-to-delivery pipeline: store setup → customer → cart → checkout → payment → fulfillment → delivery |
| `10_payments_and_fulfillment.js` | Payment capture, refund processing, shipment tracking |
| `11_b2b_operations.js` | Purchase orders, supplier management, receiving, RFQ handling |
| `12_x402_guide.js` | x402 payment intents, budget governance, settlement tracking |

## Agent Integration Examples

Located in `examples/agents/`:

| File | Description |
|------|-------------|
| `openai-embedded-toolkit.mjs` | OpenAI API integration with the embedded toolkit |
| `framework-adapters.mjs` | LangChain and Vercel AI SDK adapter patterns |
| `event-chain.js` | Event-driven agent workflow with SSE subscription |
| `workflow-example.js` | Multi-step commerce workflow orchestration |

## Multi-Agent Examples

| File | Description |
|------|-------------|
| `multi-agent/run.js` | Multiple agents coordinating on a shared commerce instance |
| `scheduled-agents.js` | Cron and interval scheduling for agent tasks |

## Gateway Examples

| File | Description |
|------|-------------|
| `gateway/examples.js` | REST API gateway integration |
| `gateway/2-multi-channel.js` | Multi-channel commerce (web + mobile + agent) |

## Daemon Examples

| File | Description |
|------|-------------|
| `daemon/quick-start.js` | Background service setup |
| `daemon/README.md` | Daemon mode documentation |

## Workflow Patterns

Documented in `examples/workflows.md`:

### Order-to-Cash Flow

```javascript
// 1. Customer places order
const order = commerce.orders.create({ customerId, items });

// 2. Payment captured
const payment = commerce.payments.create({ orderId: order.id, amount: order.total });

// 3. Inventory reserved
for (const item of items) {
    commerce.inventory.reserve(item.sku, item.quantity);
}

// 4. Order fulfilled
commerce.orders.updateStatus(order.id, 'processing');
const shipment = commerce.shipments.create({ orderId: order.id, carrier: 'FedEx' });
commerce.orders.ship(order.id);

// 5. Revenue recognized
const summary = commerce.analytics.salesSummary();
```

### Return-to-Refund Flow

```javascript
// 1. Customer requests return
const rma = commerce.returns.create({ orderId, reason: 'Defective' });

// 2. Policy evaluation
const decision = await toolkit.executeTool('evaluate_policy', {
    domain: 'returns', context: { amount: 29.99, days_since_purchase: 15 }
});

// 3. Approve and process
if (decision.allowed) {
    commerce.returns.approve(rma.id);
    commerce.payments.refund(paymentId);
    commerce.inventory.adjust(sku, 1, 'Return received');
}
```

### Procurement Flow

```javascript
// 1. Detect low stock (via heartbeat)
// 2. Find suppliers
const suppliers = commerce.suppliers.list();

// 3. Create purchase order
const po = commerce.purchaseOrders.create({
    supplierId: suppliers[0].id,
    items: [{ sku: 'WIDGET-001', quantity: 500, unitCost: 12.50 }]
});

// 4. Approve and track
commerce.purchaseOrders.approve(po.id);
// ... wait for delivery ...
commerce.purchaseOrders.receive(po.id, { receivedQuantity: 498 });
commerce.inventory.adjust('WIDGET-001', 498, 'PO received');
```

## Language Examples

Each language folder runs the same end-to-end flow (customer → product → order → ship):

- `examples/node/` — JavaScript (ES Modules)
- `examples/python/` — Python
- `examples/ruby/` — Ruby
- `examples/go/` — Go
- `examples/java/` — Java
- `examples/kotlin/` — Kotlin
- `examples/swift/` — Swift
- `examples/dotnet/` — C# / .NET

## Running Examples

```bash
# Node.js
cd examples/node
node 01_getting_started.js

# Python
cd examples/python
python basic_usage.py

# Rust
cd examples
cargo run --example basic_usage
```
