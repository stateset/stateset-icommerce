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
