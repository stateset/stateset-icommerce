# @stateset/embedded

[![npm](https://img.shields.io/npm/v/@stateset/embedded.svg)](https://www.npmjs.com/package/@stateset/embedded)
[![node](https://img.shields.io/node/v/@stateset/embedded.svg)](https://nodejs.org)

The SQLite of commerce - an embeddable commerce library powered by Rust.

## Features

- **Zero configuration** - Just point to a file path and start selling
- **Offline-first** - Works without network connectivity
- **Full-featured** - Customers, orders, products, inventory, carts, payments, returns, analytics, and currency
- **Type-safe** - Full TypeScript support with auto-generated types
- **Fast** - Native Rust performance via N-API bindings

## Installation

```bash
npm install @stateset/embedded
```

Requires Node `20.20.0+`. npm resolves your platform's prebuilt native
binary automatically via one optional dependency
(`@stateset/embedded-<platform>`, roughly 25–40 MB depending on the platform) — the install stays small
instead of bundling every platform.

**Supported platforms:** Linux x64/arm64 (glibc 2.33+ and musl),
macOS x64/arm64 (11+), Windows x64/arm64. On older glibc (e.g. Ubuntu
20.04, past EOL) use the musl build in a container, or the Python package
(`pip install stateset-embedded`), whose manylinux wheels and source
fallback reach further back.

## Development

Repo development uses the workspace-standard Node toolchain: Node `20.20.0+`
and npm `10+`. The binding package now checks that version before running
`build`, `build:debug`, `test`, `test:coverage`, `artifacts`, or `universal`.

The Rust side is one crate root (`src/lib.rs`: helpers, `Commerce`, events)
plus one file per domain under `src/domains/`. Every `npm run build*` runs
`scripts/postbuild.mjs`, which appends the hand-written declaration fragments
in `scripts/types/*.d.ts` to the napi-generated `index.d.ts`, regenerates
`tool-descriptors.json`, and regenerates the API reference under
`docs/src/api/node-reference.md`; tests fail if any of those are stale. A bare
`napi build` skips all of that — use the npm scripts.

## Quick Start

```javascript
const { Commerce } = require('@stateset/embedded');

// Open a SQLite-backed store. `open` runs migrations off the event loop;
// `new Commerce(path)` does the same work synchronously.
const commerce = await Commerce.open('./store.db');

// Or use an in-memory database for testing
// const commerce = await Commerce.open(':memory:');

// Create a customer
const customer = await commerce.customers.create({
  email: 'alice@example.com',
  firstName: 'Alice',
  lastName: 'Smith',
  phone: '+1-555-0123',
  acceptsMarketing: true
});

// Create a product
const product = await commerce.products.create({
  name: 'Premium Widget',
  description: 'A high-quality widget',
  variants: [
    { sku: 'WIDGET-001', name: 'Small', priceExact: '19.99' },
    { sku: 'WIDGET-002', name: 'Large', priceExact: '29.99' }
  ]
});

// Set up inventory
await commerce.inventory.createItem({
  sku: 'WIDGET-001',
  name: 'Small Widget',
  initialQuantity: 100,
  reorderPoint: 10
});

// Create an order
const order = await commerce.orders.create({
  customerId: customer.id,
  items: [
    { sku: 'WIDGET-001', name: 'Small Widget', quantity: 2, unitPriceExact: '19.99' }
  ],
  currency: 'USD'
});

// Ship the order
await commerce.orders.ship(order.id, 'TRACK123456');

// Analytics
const summary = await commerce.analytics.salesSummary({ period: 'last30days' });
console.log(`Revenue: $${summary.totalRevenueExact}`);

// Currency conversion (set a rate, then convert)
await commerce.currency.setRate({
  baseCurrency: 'USD',
  quoteCurrency: 'EUR',
  rate: 0.92,
  source: 'manual'
});
const conversion = await commerce.currency.convert({ from: 'USD', to: 'EUR', amount: 100 });
console.log(`$100 USD = €${conversion.convertedAmountExact} EUR`);

// Release the connection pool when you are done (also `await using`).
await commerce.close();
```

## Lifecycle

```typescript
// Asynchronous open: migrations run on a worker thread, not the event loop.
const commerce = await Commerce.open('./store.db', { maxConnections: 8 });

// Sub-API handles are stable: commerce.orders === commerce.orders.

// close() releases the engine. Calls already in flight complete; every later
// call rejects with err.code === 'PRECONDITION_FAILED'. Idempotent.
await commerce.close();
commerce.isClosed; // true

// Node 20+: `await using` closes on scope exit.
{
  await using store = await Commerce.open(':memory:');
}
```

Calls run concurrently: the engine pools its own SQLite connections, so
readers do not wait on each other or on a writer.

## Inputs are strict

A malformed input is refused with `err.code === 'VALIDATION'`; nothing is
silently coerced. An unknown currency code is an error, not the store default;
a product, customer or cart id that is not a UUID is an error, not "absent";
one bad id in a list refuses the list; a malformed date or timestamp is an
error, not "no filter". Check `err.message` for the field.

## Events

```typescript
const subscription = await commerce.events.subscribeFiltered(['order_created']);

for await (const event of subscription) {
  console.log(event.event_type, event);
  if (done) break; // leaving the loop closes the subscription
}

// Or pull one at a time: `null` once the stream has ended.
const next = await subscription.recv();
subscription.close();
```

An open subscription never keeps the process alive by itself, so a script
that finishes its work exits promptly even with a `recv()` outstanding. Call
`subscription.ref()` when the subscription *should* keep the process running
(a worker whose only job is to react to events), and `unref()` to undo that.
Closing the `Commerce` ends its subscriptions.

## Agent Framework Embedding

The adapter subpaths work with nothing but this package installed:

- `@stateset/embedded/openai` — OpenAI tool definitions + tool-call execution
- `@stateset/embedded/generic` — framework-neutral `{ name, description, schema, execute }` descriptors
- `@stateset/embedded/langchain` — `DynamicStructuredTool` instances
- `@stateset/embedded/vercel-ai` — Vercel AI SDK `tool()` map
- `@stateset/embedded/native-toolkit` — the toolkit behind them (`getTools({ format })`, `executeTool`, …)

They are backed by `tool-descriptors.json`, a catalog generated from
`index.d.ts` at build time: one tool per public method reachable from a
`Commerce` getter, named `<getter>.<method>` (`orders.create`,
`customers.getByEmail`, `analytics.salesSummary`), with a JSON Schema built
from the declared input types. On the wire (OpenAI, Anthropic, MCP) the name
is `orders__create`, because those formats forbid `.`; both spellings are
accepted everywhere a name is passed in.

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';
import { createToolDescriptors } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');
const openaiTools = createOpenAITools(commerce, {
  filter: ['customers.list', 'orders.get', 'orders.create'],
});
const execution = await executeOpenAIToolCall(commerce, {
  call_id: 'demo_call_1',
  function: { name: 'customers__list', arguments: '{}' },
});
const descriptors = createToolDescriptors(commerce, {
  filter: ['orders.*', 'analytics.salesSummary'],
});
```

**Writes are preview-only by default.** A read tool (`get*`, `list*`,
`count*`, `find*`, `search*`, `calculate*`, `validate*`, `is*`, …) executes
immediately. A write tool returns `{ preview: true, tool, params, note }` and
touches nothing until you opt in with `allowApply: true` — the same posture as
`--apply` on the CLI and MCP server. Engine failures come back as
`{ error: { code, message, details } }` with the binding's `err.code`
(`NOT_FOUND`, `VALIDATION`, …), so a tool-calling loop never has to catch.

```javascript
import { createNativeToolkit } from '@stateset/embedded/native-toolkit';

const toolkit = createNativeToolkit(commerce, { allowApply: true, filter: ['orders.*'] });
toolkit.getTools({ format: 'mcp' });          // also 'openai', 'anthropic', 'generic'
await toolkit.executeTool('orders.create', { input: { customerId, items } });
```

### What `@stateset/cli` adds

`@stateset/cli` is an **optional peer dependency** — the engine never pulls
it in. When it is installed, the same adapter calls transparently switch to
its agent toolkit (`toolkit.backend === 'cli'` instead of `'native'`), which
adds the governed runtime: capability scopes, policy evaluation, spend
budgets, mutation simulation and replay logs, kernel-governed commands, x402
/ MPP payments, and the 900+ curated tools the MCP server exposes
(`list_customers`-style names). Install it alongside the binding when you
want that, and use the dedicated entrypoint for the full surface:

```bash
npm install @stateset/embedded @stateset/cli
```

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/embedded/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const openaiTools = toolkit.getTools({ format: 'openai' });
const mcpTools = toolkit.getTools({ format: 'mcp' });
```

Pass `backend: 'native'` to `resolveToolkit` (from `toolkit-helpers.mjs`) to
force the built-in toolkit even when the CLI is present.

## Money: use the `*Exact` fields

A JavaScript `number` is a double, and a double cannot hold `19.99`. By the time
a price reaches this binding as a `number` it is already `19.989999999999998`;
JavaScript prints it as `19.99` because it prints the shortest string that
round-trips, not because the value is right. So every money value crosses this
boundary twice:

- `total`, `unitPrice`, `subtotal`, … — the `number`, kept for compatibility and
  marked `@deprecated`. Float money will be removed in 2.0.
- `totalExact`, `unitPriceExact`, `subtotalExact`, … — a **string** carrying the
  engine's exact base-10 `Decimal`, with no float anywhere in the path. `0.10`
  plus `0.20` is `"0.30"` here, never `0.30000000000000004`.

Read the `*Exact` field for anything you will store, compare, total or show a
customer. Read the `number` only where an approximation is genuinely fine.

The string carries the engine's scale, not a currency's display format: `2 ×
12.50` renders as `"25.0"`, not `"25.00"`. It is exact either way, so compare two
amounts numerically (parse them, or use a decimal library) rather than by string
equality — `"25.0" !== "25.00"` even though the amounts are the same.

```typescript
const order = await commerce.orders.get(id);

order.totalAmount;                     // prints 59.97 …
order.totalAmount.toPrecision(17);     // … but the value is 59.969999999999999
order.totalAmountExact;                // "59.97" — the actual amount
```

Inputs work the same way, in reverse. Every money input accepts a
`<field>Exact` string, and the float `<field>` is optional: send one or the
other. The string wins when both are sent, and sending neither is a
`VALIDATION` error naming the field.

```typescript
await commerce.orders.create({
  customerId,
  items: [
    { sku: 'WIDGET', name: 'Widget', quantity: 3, unitPriceExact: '19.99' },
  ],
});
// -> items[0].totalExact === "59.97"
```

`orders.createExact`, `payments.createExact`, `payments.createRefundExact` and
`carts.addItemExact` are the fully string-typed variants, for callers that would
rather not carry the float half at all.

A money value the engine cannot narrow to a `number` no longer becomes `NaN`: it
throws with `err.code === 'INTERNAL'` and a message naming the field.
`test/fixtures/money-fields.json` is the census of which fields are money, and
`test/money-exactness.js` fails if a new float money field is added without a
twin.

## API Reference

The full surface — every sub-API on `Commerce`, each method signature with its
JSDoc, the crypto / VES / x402 functions, and every interface and type alias in
`index.d.ts` — is in the generated
[Node.js API Reference](../../docs/src/api/node-reference.md)
(published at [docs.stateset.com](https://docs.stateset.com/api/node-reference.html)).
It is rebuilt from `index.d.ts` by `scripts/generate-api-reference.mjs` on
every build, and `test/api-reference.js` fails when the committed page drifts.
The snippets below are the operations most integrations start with.

### Commerce

Main entry point for all commerce operations.

```typescript
const commerce = await Commerce.open(dbPath: string, options?: { maxConnections?: number });
const commerce = new Commerce(dbPath: string); // synchronous alternative
await commerce.close();
```

### Customers and orders

```typescript
const customer = await commerce.customers.findOrCreate({
  email: 'alice@example.com',
  firstName: 'Alice',
  lastName: 'Smith',
});

// Exact money in, exact money out: `createExact` never touches a float.
const order = await commerce.orders.createExact({
  customerId: customer.id,
  items: [{ sku: 'SKU-001', name: 'Widget', quantity: 2, unitPrice: '29.99' }],
  currency: 'USD',
});
console.log(order.totalAmountExact); // '59.98'

const shipped = await commerce.orders.ship(order.id, '1Z999AA10123456784');
const orders = await commerce.orders.list();
```

### Inventory

```typescript
const item = await commerce.inventory.createItem({ sku: 'SKU-001', name: 'Widget', initialQuantity: 100 });
const stock = await commerce.inventory.getStock('SKU-001');

await commerce.inventory.adjust('SKU-001', -5, 'damaged');

// Hold stock for an order, then confirm or release it.
const reservation = await commerce.inventory.reserve('SKU-001', 2, 'order', order.id, 900);
await commerce.inventory.confirmReservation(reservation.id);
```

### Carts / Checkout

```typescript
const cart = await commerce.carts.create({ customerEmail: 'alice@example.com', currency: 'USD' });
await commerce.carts.addItem(cart.id, { sku: 'SKU-001', name: 'Widget', quantity: 2, unitPriceExact: '29.99' });
await commerce.carts.setShipping(cart.id, {
  shippingAddress: { firstName: 'Alice', lastName: 'Smith', line1: '123 Main St', city: 'San Francisco', postalCode: '94105', country: 'US' },
  shippingMethod: 'standard',
  shippingAmountExact: '9.99',
});

await commerce.carts.reserveInventory(cart.id);
const result = await commerce.carts.complete(cart.id); // creates the order
console.log(result.orderNumber);
```

## TypeScript

This package includes TypeScript definitions out of the box. All types are automatically generated from the Rust source code.

```typescript
import { Commerce, CustomerOutput, OrderOutput } from '@stateset/embedded';

const commerce = new Commerce('./store.db');
const customer: CustomerOutput = await commerce.customers.create({...});
```

## Database

The library uses SQLite under the hood with automatic schema migrations. Your data is stored in a single file that you specify when creating the Commerce instance.

- Use a file path like `./store.db` for persistent storage
- Use `:memory:` for in-memory databases (great for testing)

## License

MIT OR Apache-2.0
