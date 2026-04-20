# Examples

Runnable examples live in the `examples/` directory. Each demonstrates a specific workflow or integration pattern.

## Node.js Examples

Located in `examples/node/`:

| File | Description |
|------|-------------|
| `examples/node/basic_usage.js` | Minimal single-file commerce flow for the Node binding |
| `examples/node/01_getting_started.js` | Basic Commerce instance setup, customer/product/order creation |
| `examples/node/02_cart_and_checkout.js` | Cart management, item operations, discount application, checkout flow |
| `examples/node/03_analytics_and_forecasting.js` | Revenue summaries, demand prediction, inventory health |
| `examples/node/04_subscriptions.js` | Recurring billing: plan creation, subscribe, pause, resume, cancel, churn analysis |
| `examples/node/05_promotions.js` | Discounts, coupon codes, campaign management |
| `examples/node/06_currency.js` | Multi-currency conversion, exchange rate management, 150+ currencies |
| `examples/node/07_tax.js` | Multi-jurisdiction tax (US states, EU VAT, Canadian GST/PST/HST), nexus detection |
| `examples/node/08_manufacturing.js` | BOM creation, work orders, quality control, yield tracking |
| `examples/node/09_full_workflow.js` | Complete order-to-delivery pipeline: store setup -> customer -> cart -> checkout -> payment -> fulfillment -> delivery |
| `examples/node/10_payments_and_fulfillment.js` | Payment capture, refund processing, shipment tracking |
| `examples/node/11_b2b_operations.js` | Purchase orders, supplier management, receiving, RFQ handling |
| `examples/node/x402_guide.js` | x402 payment intents, budget governance, settlement tracking |

## Agent Integration Examples

Located in `examples/agents/` and `examples/python/`:

| File | Description |
|------|-------------|
| `examples/agents/openai-embedded-toolkit.mjs` | OpenAI-style tool loop using `@stateset/embedded/openai` |
| `examples/agents/custom-framework-adapter.mjs` | Framework-neutral descriptor and callable-registry pattern using `@stateset/embedded/generic` |
| `examples/agents/framework-adapters.mjs` | LangChain and Vercel AI SDK adapter patterns using `@stateset/embedded/langchain` and `@stateset/embedded/vercel-ai` |
| `examples/agents/embedded-toolkit-runtime.mjs` | Shared runtime helper used by the JS embedded-toolkit examples |
| `examples/agents/event-chain.js` | Event-driven agent workflow with SSE subscription |
| `examples/agents/workflow-example.js` | Multi-step commerce workflow orchestration |
| `examples/agents/x402-demo-flows.mjs` | Runnable x402 demo catalog covering paid HTTP, local intents, and credit-ledger flows |
| `examples/agents/x402-local-intent-flow.mjs` | Local x402 intent flow with embedded settlement primitives |
| `examples/agents/x402-exact-http-flow.mjs` | Exact-amount paid HTTP flow for agent checkout and settlement demos |
| `examples/agents/x402-credit-ledger-flow.mjs` | Metered credit-ledger flow for embedded agent commerce |
| `examples/agents/README.md` | Guide to the x402 agent demos and the shared demo helper surfaces |
| `examples/python/agent_toolkit.py` | Native Python agent-toolkit example for OpenAI-compatible and framework-neutral runtimes |
| `examples/python/openai_tools.py` | Focused OpenAI-compatible Python example using `stateset_embedded.openai` |
| `examples/python/generic_tools.py` | Focused framework-neutral Python example using `stateset_embedded.generic` |
| `examples/python/langchain_tools.py` | Focused LangChain Python example using `stateset_embedded.langchain` |
| `examples/python/crewai_tools.py` | Focused CrewAI Python example using `stateset_embedded.crewai` |
| `examples/python/autogen_tools.py` | Focused AutoGen Python example using `stateset_embedded.autogen` |
| `examples/python/framework_adapters.py` | Native Python framework-module patterns for LangChain, CrewAI, and AutoGen-style runtimes |

## Multi-Agent Examples

| File | Description |
|------|-------------|
| `examples/multi-agent/run.js` | Multiple agents coordinating on a shared commerce instance |
| `examples/scheduled-agents.js` | Cron and interval scheduling for agent tasks |

## Gateway Examples

| File | Description |
|------|-------------|
| `examples/gateway/examples.js` | REST API gateway integration |
| `examples/gateway/2-multi-channel.js` | Multi-channel commerce (web + mobile + agent) |
| `examples/gateway-examples/3-http-gateway.js` | HTTP gateway walkthrough with sample config and endpoint patterns |

## Daemon Examples

| File | Description |
|------|-------------|
| `examples/daemon/quick-start.js` | Background service setup |
| `examples/daemon/README.md` | Daemon mode documentation |

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

# JS embedded agent examples
cd ../agents
node openai-embedded-toolkit.mjs
node custom-framework-adapter.mjs
node framework-adapters.mjs

# Python
cd examples/python
python basic_usage.py
python openai_tools.py
python generic_tools.py
python langchain_tools.py
python crewai_tools.py
python autogen_tools.py
python framework_adapters.py

# Rust
cd examples
cargo run --example basic_usage
```
