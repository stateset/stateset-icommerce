/**
 * Lifecycle tools tell agents the truth about the engine's state machines,
 * through the real MCP server on a fresh SQLite store:
 *
 *   - a refused catalogued move carries `allowedNext` and a hint naming the
 *     tool for each allowed move -- on `executeTool` and on the MCP response
 *     (structured content and text), for kernel receipts and thrown engine
 *     errors alike;
 *   - a preview of a catalogued move says which states the engine accepts it
 *     from (no --apply, nothing read or written), and a kernel preview that
 *     would be refused carries the same guidance as the refusal;
 *   - reads of a lifecycle record carry `allowedNext` for its current status.
 *
 * Nothing here changes which moves the engine accepts: every refusal below is
 * the engine's own.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../src/kernel-tool-execution.js';
import { validateToolResultContract } from '../../src/mcp/tool-result-contract.js';

const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-lifecycle-ax-'));
const dbPath = path.join(dir, 'store.db');
const capabilities = [...new Set(Object.values(KERNEL_CAPABILITY_BY_TOOL))];
const kernel = {
  strict: false,
  storeId: 'store:lifecycle',
  principal: {
    id: 'agent:lifecycle',
    kind: 'agent',
    tenantId: 'tenant:lifecycle',
    delegatedBy: 'user:lifecycle',
    capabilities,
  },
  policy: {
    version: 'lifecycle-v1',
    commands: Object.fromEntries(
      capabilities.map((capability) => [
        capability,
        {
          required_capabilities: [capability],
          requires_approval: false,
          requires_tenant: true,
          requires_store: true,
          allowed_tenant_ids: ['tenant:lifecycle'],
          allowed_store_ids: ['store:lifecycle'],
          requires_agent_delegation: true,
          requires_signed_authority: false,
        },
      ]),
    ),
    trusted_authority_keys: {},
  },
};

let server;
let previewServer;
const ids = {};

/** Call a tool that must succeed; return its result. */
async function call(name, params) {
  const out = await server.executeTool(name, params);
  assert.equal(out.ok, true, `${name}: ${JSON.stringify(out.failure)}`);
  return out.result;
}

/** A governed tool answers with its receipt's result; a legacy tool with its own shape. */
const entity = (result, key) => result[key] ?? result.receipt?.result ?? result.result;

const adapted = (target, name) => {
  const tool = target.getAdaptedTools().find((t) => t.name === name);
  assert.ok(tool, `${name} is exposed`);
  return tool;
};

before(async () => {
  server = createStatesetMcpServer({ dbPath, allowApply: true, kernel });
  previewServer = createStatesetMcpServer({ dbPath: path.join(dir, 'preview.db') });

  const customer = entity(
    await call('create_customer', { email: 'ada@example.com', firstName: 'Ada', lastName: 'L' }),
    'customer',
  );
  ids.customer = customer.id;
  const order = (
    await call('create_order', {
      customerId: customer.id,
      items: [{ sku: 'W-1', name: 'Widget', quantity: 2, unitPrice: 5 }],
    })
  ).order;
  ids.order = order.id;
  ids.orderItem = (await call('get_order', { identifier: order.id })).order.items[0].id;
  await call('update_order_status', { orderId: order.id, status: 'confirmed' });
  await call('update_order_status', { orderId: order.id, status: 'processing' });

  // A return needs a shipped order.
  const shipped = (
    await call('create_order', {
      customerId: customer.id,
      items: [{ sku: 'W-2', name: 'Widget 2', quantity: 1, unitPrice: 7 }],
    })
  ).order;
  ids.shippedOrder = shipped.id;
  const shippedItem = (await call('get_order', { identifier: shipped.id })).order.items[0].id;
  await call('update_order_status', { orderId: shipped.id, status: 'confirmed' });
  await call('update_order_status', { orderId: shipped.id, status: 'processing' });
  await call('ship_order', { orderId: shipped.id, trackingNumber: '1Z-LIFECYCLE' });
  ids.return = entity(
    await call('create_return', {
      orderId: shipped.id,
      reason: 'defective',
      items: [{ orderItemId: shippedItem, quantity: 1 }],
    }),
    'return',
  ).id;
  await call('approve_return', { returnId: ids.return });

  ids.payment = entity(
    await call('create_payment', { orderId: order.id, amount: '10.00' }),
    'payment',
  ).id;
  ids.shipment = entity(
    await call('create_shipment', {
      orderId: order.id,
      carrier: 'ups',
      recipientName: 'Ada L',
      shippingAddress: '1 Main St, Los Angeles, CA 90001, US',
    }),
    'shipment',
  ).id;
  ids.cart = entity(await call('create_cart', { customerEmail: 'ada@example.com' }), 'cart').id;
  await call('expire_cart', { cartId: ids.cart });
  ids.product = entity(
    await call('create_product', { name: 'Gadget', variants: [{ sku: 'G-1', price: '9.00' }] }),
    'product',
  ).id;
});

after(async () => {
  for (const target of [server, previewServer]) {
    try {
      await target?.close?.();
    } catch {
      // never connected to a transport
    }
  }
  rmSync(dir, { recursive: true, force: true });
});

describe('a refused catalogued move says what is allowed next', () => {
  it('on a kernel receipt, through executeTool and the MCP response', async () => {
    const refused = await server.executeTool('mark_return_received', { returnId: ids.return });
    assert.equal(refused.ok, false);
    assert.equal(refused.failure.code, 'commerce.invalid_return_status_transition');
    assert.equal(refused.failure.message, 'return cannot transition from approved to received');
    assert.deepEqual(refused.failure.allowedNext, ['in_transit', 'cancelled']);
    assert.deepEqual(refused.failure.transition, {
      machine: 'return',
      from: 'approved',
      to: 'received',
    });
    assert.match(refused.failure.hint, /in_transit \(add_return_tracking\)/);
    assert.match(refused.failure.hint, /approved -> in_transit -> received/);

    const response = await adapted(server, 'mark_return_received').handler(
      { returnId: ids.return },
      {},
    );
    assert.equal(response.isError, true);
    assert.deepEqual(validateToolResultContract(response.structuredContent), []);
    assert.deepEqual(response.structuredContent.error.allowedNext, ['in_transit', 'cancelled']);
    // Text-only clients see the receipt and the guidance.
    const text = JSON.parse(response.content[0].text);
    assert.equal(text.receipt.status, 'rejected');
    assert.deepEqual(text.allowedNext, ['in_transit', 'cancelled']);
    assert.match(text.hint, /add_return_tracking/);
  });

  it('on an order move the kernel refuses', async () => {
    const refused = await server.executeTool('update_order_status', {
      orderId: ids.order,
      status: 'delivered',
    });
    assert.equal(refused.failure.code, 'commerce.invalid_order_status_transition');
    assert.deepEqual(refused.failure.allowedNext, ['partially_shipped', 'shipped', 'cancelled']);
    assert.match(refused.failure.hint, /shipped \(ship_order\)/);
  });

  it('on a thrown engine error, through executeTool and the MCP response', async () => {
    const refused = await server.executeTool('cancel_cart', { cartId: ids.cart });
    assert.equal(refused.ok, false);
    assert.equal(refused.failure.code, 'CONFLICT');
    assert.deepEqual(refused.failure.allowedNext, []);
    assert.deepEqual(refused.failure.transition, {
      machine: 'cart',
      from: 'expired',
      to: 'cancelled',
    });
    assert.match(refused.failure.hint, /A cart in expired is final/);

    const response = await adapted(server, 'cancel_cart').handler({ cartId: ids.cart }, {});
    assert.equal(response.isError, true);
    assert.deepEqual(response.structuredContent.error.allowedNext, []);
    const text = JSON.parse(response.content[0].text);
    assert.equal(text.code, 'CONFLICT');
    assert.deepEqual(text.allowedNext, []);
    assert.match(text.hint, /is final/);
  });

  it('on a shipment move the engine refuses', async () => {
    const refused = await server.executeTool('deliver_shipment', { shipmentId: ids.shipment });
    assert.deepEqual(refused.failure.allowedNext, ['processing', 'cancelled', 'on_hold']);
    assert.match(refused.failure.hint, /processing \(update_shipment\)/);
    assert.match(refused.failure.hint, /To reach delivered: pending -> processing/);
  });

  it('leaves failures that are not status moves as they were', async () => {
    const missing = await server.executeTool('get_payment', { paymentId: 'not-a-uuid' });
    assert.equal(missing.ok, false);
    assert.equal(missing.failure.allowedNext, undefined);
    assert.equal(missing.failure.transition, undefined);
  });
});

describe('reads carry allowedNext for the current status', () => {
  it('get_order, get_return, get_shipment, get_cart, get_payment, get_customer, get_product', async () => {
    const reads = [
      [
        'get_order',
        { identifier: ids.order },
        'order',
        ['partially_shipped', 'shipped', 'cancelled'],
      ],
      ['get_return', { returnId: ids.return }, 'return', ['in_transit', 'cancelled']],
      [
        'get_shipment',
        { shipmentId: ids.shipment },
        'shipment',
        ['processing', 'cancelled', 'on_hold'],
      ],
      ['get_cart', { identifier: ids.cart }, 'cart', []],
      // A pending payment can be completed in one step (the persistence-layer move).
      [
        'get_payment',
        { paymentId: ids.payment },
        'payment',
        ['processing', 'failed', 'cancelled', 'completed'],
      ],
      [
        'get_customer',
        { identifier: ids.customer },
        'customer',
        ['inactive', 'suspended', 'deleted'],
      ],
      ['get_product', { productId: ids.product }, 'product', ['active', 'archived']],
    ];
    for (const [tool, params, key, allowedNext] of reads) {
      const result = await call(tool, params);
      assert.ok(result[key]?.status, `${tool} returns the ${key}`);
      assert.deepEqual(result.allowedNext, allowedNext, tool);
    }
  });

  it('explain_order', async () => {
    const result = await call('explain_order', { orderId: ids.order });
    assert.equal(result.order.status, 'processing');
    assert.deepEqual(result.allowedNext, ['partially_shipped', 'shipped', 'cancelled']);
  });
});

/** A fresh processing order with `quantity` units of one line. */
async function processingOrder(quantity) {
  const order = (
    await call('create_order', {
      customerId: ids.customer,
      items: [{ sku: `P-${quantity}`, name: 'Part', quantity, unitPrice: 3 }],
    })
  ).order;
  await call('update_order_status', { orderId: order.id, status: 'confirmed' });
  await call('update_order_status', { orderId: order.id, status: 'processing' });
  const item = (await call('get_order', { identifier: order.id })).order.items[0];
  return { id: order.id, itemId: item.id };
}

describe('order and customer statuses match what the engine accepts', () => {
  it('update_order_status refuses shipped and pending by schema, naming what to do', async () => {
    for (const [status, pattern] of [
      ['shipped', /call ship_order/],
      ['partially_shipped', /ship_order with `lines`/],
      ['pending', /nothing moves back to pending/],
    ]) {
      const out = await server.executeTool('update_order_status', { orderId: ids.order, status });
      assert.equal(out.ok, false, status);
      assert.equal(out.failure.code, 'INVALID_INPUT', status);
      assert.match(out.failure.hint, pattern, status);
    }
    const tool = server.getAdaptedTools().find((t) => t.name === 'update_order_status');
    assert.match(tool.description, /cancelled \(from pending, confirmed or processing\)/);
    assert.match(tool.description, /call ship_order/);
  });

  it('ship_order takes lines, so partially_shipped is reachable', async () => {
    const order = await processingOrder(3);
    const partial = await call('ship_order', {
      orderId: order.id,
      trackingNumber: '1Z-PART',
      lines: [{ orderItemId: order.itemId, quantity: 1 }],
    });
    assert.equal(entity(partial, 'order').status, 'partially_shipped');
    const read = await call('get_order', { identifier: order.id });
    assert.equal(read.order.status, 'partially_shipped');
    assert.deepEqual(read.allowedNext, ['shipped']);

    // Shipping more than is left is the engine's refusal, not a silent full shipment.
    const tooMany = await server.executeTool('ship_order', {
      orderId: order.id,
      lines: [{ orderItemId: order.itemId, quantity: 5 }],
    });
    assert.equal(tooMany.ok, false);

    const rest = await call('ship_order', { orderId: order.id });
    assert.equal(entity(rest, 'order').status, 'shipped');
  });

  it('cancel_order cancels a processing order, as its description now says', async () => {
    const tool = server.getAdaptedTools().find((t) => t.name === 'cancel_order');
    assert.match(tool.description, /from pending, confirmed or processing/);
    const order = await processingOrder(1);
    const cancelled = await call('cancel_order', { orderId: order.id });
    assert.equal(cancelled.order.status, 'cancelled');
  });

  it('update_customer refuses deleted by schema and points to delete_customer', async () => {
    const customer = entity(
      await call('create_customer', { email: 'grace@example.com', firstName: 'G', lastName: 'H' }),
      'customer',
    );
    const refused = await server.executeTool('update_customer', {
      customerId: customer.id,
      status: 'deleted',
    });
    assert.equal(refused.failure.code, 'INVALID_INPUT');
    assert.match(refused.failure.hint, /call delete_customer/);

    const suspended = await call('update_customer', {
      customerId: customer.id,
      status: 'suspended',
    });
    assert.equal(suspended.customer.status, 'suspended');
    await call('delete_customer', { customerId: customer.id });
    const read = await call('get_customer', { identifier: customer.id });
    assert.equal(read.customer.status, 'deleted');
    assert.deepEqual(read.allowedNext, []);
  });
});

describe('status filters offer what is stored, and older values still work', () => {
  const statusesOf = async (tool, params, key) =>
    (await call(tool, params))[key].map((record) => record.status);

  it('promotions and coupons use the engine vocabulary (exhausted, archived)', async () => {
    for (const status of ['exhausted', 'archived', 'draft']) {
      assert.deepEqual(await statusesOf('list_promotions', { status }, 'promotions'), [], status);
    }
    // `depleted` used to be offered and made the binding throw; it now means exhausted.
    for (const status of ['exhausted', 'depleted']) {
      const out = await server.executeTool('list_coupons', { status });
      assert.equal(out.ok, true, `${status}: ${JSON.stringify(out.failure)}`);
    }
  });

  it('payment intents: stored values, the cancelled alias, and an explained refusal', async () => {
    const manual = (
      await call('create_payment_intent', { amount: '12.00', captureMethod: 'manual' })
    ).intent;
    const cancelled = (
      await call('create_payment_intent', { amount: '13.00', captureMethod: 'manual' })
    ).intent;
    await call('cancel_payment_intent', { intentId: cancelled.id });
    const ids = async (status) =>
      (await call('list_payment_intents', { status })).intents.map((intent) => intent.id);

    assert.ok((await ids('requires_capture')).includes(manual.id));
    assert.ok((await ids('canceled')).includes(cancelled.id));
    assert.deepEqual(await ids('cancelled'), await ids('canceled'), 'cancelled is an alias');
    const reconciled = await call('reconcile_payment_provider', { status: 'cancelled' });
    assert.ok(JSON.stringify(reconciled).includes(cancelled.id));

    const pending = await server.executeTool('list_payment_intents', { status: 'pending' });
    assert.equal(pending.failure.code, 'INVALID_INPUT');
    assert.match(pending.failure.hint, /requires_capture/);
  });

  it('settlements and batches: paid, with completed as an alias', async () => {
    const intent = (
      await call('create_payment_intent', { amount: '20.00', captureMethod: 'automatic' })
    ).intent;
    const created = await call('create_payment_settlement_batch', { intentIds: [intent.id] });
    assert.equal(created.batch.status, 'paid');
    for (const status of ['paid', 'completed']) {
      const settlements = (await call('list_payment_settlements', { status })).settlements;
      assert.ok(
        settlements.some((s) => s.intentId === intent.id),
        status,
      );
      const batches = (await call('list_payment_settlement_batches', { status })).batches;
      assert.ok(
        batches.some((b) => b.id === created.batch.id),
        status,
      );
    }
    const processing = await server.executeTool('list_payment_settlement_batches', {
      status: 'processing',
    });
    assert.equal(processing.failure.code, 'INVALID_INPUT');
    assert.match(processing.failure.hint, /recorded as paid/);
  });

  it('shipping labels: label_created and voided, with created/purchased/error aliases', async () => {
    const { rates } = await call('quote_shipping_rates', {
      originAddress: {
        line1: '1 Main St',
        city: 'Los Angeles',
        state: 'CA',
        postalCode: '90001',
        country: 'US',
      },
      destinationAddress: {
        line1: '2 Broadway',
        city: 'New York',
        state: 'NY',
        postalCode: '10001',
        country: 'US',
      },
      parcels: [{ weightGrams: 500 }],
    });
    const label = (await call('create_shipping_label', { rateId: rates[0].rateId })).label;
    assert.equal(label.status, 'label_created');
    for (const status of ['label_created', 'created', 'purchased']) {
      const labels = (await call('list_shipping_labels', { status })).labels;
      assert.ok(
        labels.some((l) => l.id === label.id),
        status,
      );
    }
    await call('void_shipping_label', { labelId: label.id });
    const voided = (await call('list_shipping_labels', { status: 'voided' })).labels;
    assert.ok(voided.some((l) => l.id === label.id));
    assert.equal((await server.executeTool('list_shipping_labels', { status: 'error' })).ok, true);
  });

  it('tax transactions: committed/adjusted/voided; pending is explained', async () => {
    const quote = (
      await call('calculate_tax_quote', {
        items: [{ unitPrice: '10.00', quantity: 1 }],
        shippingAddress: { country: 'US', state: 'CA', postalCode: '90001' },
      })
    ).quote;
    const committed = (await call('commit_tax_transaction', { quoteId: quote.id })).transaction;
    const listed = (await call('list_tax_transactions', { status: 'committed' })).transactions;
    assert.ok(listed.some((t) => t.id === committed.id));
    assert.equal(
      (await server.executeTool('list_tax_transactions', { status: 'adjusted' })).ok,
      true,
    );
    const pending = await server.executeTool('list_tax_transactions', { status: 'pending' });
    assert.equal(pending.failure.code, 'INVALID_INPUT');
    assert.match(pending.failure.hint, /get_tax_quote/);
  });

  it('payment links: expired is derived from the expiry, not stored', async () => {
    const lapsing = await call('create_payment_link', {
      items: [{ name: 'Gift', quantity: 1, unitPrice: 5 }],
      currency: 'USD',
      expiresIn: 1,
    });
    const lasting = await call('create_payment_link', {
      items: [{ name: 'Gift', quantity: 1, unitPrice: 5 }],
      currency: 'USD',
    });
    await new Promise((resolve) => setTimeout(resolve, 1200));
    const expired = (await call('list_payment_links', { status: 'expired' })).links;
    assert.deepEqual(
      expired.map((link) => [link.id, link.status, link.expired]),
      [[lapsing.linkId, 'active', true]],
    );
    const active = (await call('list_payment_links', { status: 'active' })).links;
    assert.ok(active.some((link) => link.id === lasting.linkId));
    assert.ok(!active.some((link) => link.id === lapsing.linkId));
  });
});

describe('a preview of a catalogued move states its precondition', () => {
  it('without --apply, on executeTool and the MCP response', async () => {
    const preview = await previewServer.executeTool('mark_return_received', { returnId: 'r-1' });
    assert.equal(preview.ok, true);
    assert.equal(preview.preview, true);
    assert.match(preview.notice.message, /^Preview mode: would execute 'mark_return_received'/);
    assert.match(
      preview.notice.message,
      /only for a return in in_transit \(reached via add_return_tracking\)/,
    );
    assert.deepEqual(preview.wouldDo.transition, {
      machine: 'return',
      to: 'received',
      acceptedFrom: ['in_transit'],
    });

    const response = await adapted(previewServer, 'update_order_status').handler(
      { orderId: 'o-1', status: 'refunded' },
      {},
    );
    assert.notEqual(response.isError, true);
    assert.equal(response.structuredContent.preview, true);
    assert.match(response.structuredContent.notice.message, /only for an order in delivered/);
    const text = JSON.parse(response.content[0].text);
    assert.deepEqual(text.wouldDo.transition.acceptedFrom, ['delivered']);
  });

  it('a kernel preview that would be refused carries the refusal guidance', async () => {
    await server.close?.();
    server = null;
    const kernelPreview = createStatesetMcpServer({ dbPath, kernel });
    try {
      const out = await kernelPreview.executeTool('mark_return_received', { returnId: ids.return });
      assert.equal(out.ok, false);
      assert.equal(out.preview, true);
      assert.equal(out.failure.code, 'commerce.invalid_return_status_transition');
      assert.deepEqual(out.failure.allowedNext, ['in_transit', 'cancelled']);
    } finally {
      await kernelPreview.close?.();
    }
  });
});
