import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {
  ShipmentOperations,
  shipmentAction,
  shipmentReceiptIssues,
} from '../../src/tui/shipment-operations.js';
import { shipmentCommitment } from '../../src/tui/shipment-engine.js';
import { initialState, navigate, handleKey, paletteCommands } from '../../src/tui/model.js';
import { render } from '../../src/tui/render.js';
import { shipmentWorkflowLines } from '../../src/tui/shipment-workflow.js';
import { shipmentKernel } from '../fixtures/tui-shipments.js';
const id = 'b734166a-c7c8-42ae-b991-eb1974a3f13f',
  lineId = 'e29e51a4-299c-4f0f-b738-f8adad0b7964';
const order = {
  id,
  orderNumber: 'ORD-SHIP',
  version: 3,
  items: [{ id: lineId, sku: 'STOCK', quantity: 2, shippedQuantity: 0 }],
};
const params = {
  orderId: id,
  trackingNumber: 'TRACK',
  lines: [{ orderItemId: lineId, quantity: 1 }],
};
const context = {
  trackingNumber: 'TRACK',
  status: 'partially_shipped',
  lines: [{ id: lineId, quantity: 2, shippedQuantity: 1 }],
};
function receipt(status, key) {
  return {
    command_id: lineId,
    command_type: 'orders.ship_reserved',
    idempotency_key: key,
    status,
    aggregate_id: id,
    version_before: 3,
    version_after: status === 'succeeded' ? 4 : null,
    economic_context: { principal: { id: 'user:tui' }, store_id: 'store:tui' },
    result:
      status === 'succeeded'
        ? {
            id,
            version: 4,
            status: 'partially_shipped',
            tracking_number: 'TRACK',
            items: [{ id: lineId, quantity: 2, shipped_quantity: 1 }],
          }
        : null,
  };
}
const key = (state, text = '', name = text, extra = {}) =>
  handleKey(state, text, { name, ...extra });
test('shipment input bounds lines, quantities, tracking and rejects caller-owned execution authority', () => {
  assert.deepEqual(shipmentAction(params), params);
  for (const change of [
    { orderId: 'bad' },
    { trackingNumber: 'bad\ntracking' },
    { trackingNumber: 'x'.repeat(201) },
    { lines: [] },
    { lines: [...params.lines, ...params.lines] },
    { lines: [{ orderItemId: lineId, quantity: 0 }] },
    { lines: [{ orderItemId: lineId, quantity: 1.5 }] },
    { lines: [{ orderItemId: lineId, quantity: 2147483648 }] },
    { expectedVersion: 1 },
    { principal: {} },
    { idempotencyKey: 'evil' },
  ])
    assert.throws(() => shipmentAction({ ...params, ...change }));
});
test('shipment commitment binds exact reviewed units and preserves operator restrictions', () => {
  const kernel = {
    commitment: { quantity: '1.0', evidence: ['dispatch'], counterparty_id: 'warehouse' },
  };
  assert.deepEqual(shipmentCommitment(kernel, params.lines), {
    quantity: '1',
    evidence: ['dispatch'],
    counterparty_id: 'warehouse',
  });
  assert.equal(kernel.commitment.quantity, '1.0');
  assert.throws(
    () => shipmentCommitment({ commitment: { quantity: '2' } }, params.lines),
    /does not match/,
  );
});
test('receipt review detects wrong line quantities, tracking, order, version and scope', () => {
  const review = {
    params,
    context,
    expectedVersion: 3,
    principalId: 'user:tui',
    storeId: 'store:tui',
  };
  assert.deepEqual(shipmentReceiptIssues(receipt('succeeded', 'key'), review), []);
  for (const mutate of [
    (r) => (r.result.items[0].shipped_quantity = 2),
    (r) => (r.result.tracking_number = 'OTHER'),
    (r) => (r.result.id = lineId),
    (r) => (r.version_before = 2),
    (r) => (r.economic_context.store_id = 'other'),
    (r) => (r.result.items = []),
  ]) {
    const r = receipt('succeeded', 'key');
    mutate(r);
    assert.ok(shipmentReceiptIssues(r, review).length);
  }
});
test('shipment editing owns keys, allows partial dispatch, requires typed APPLY and preserves confirmation across layouts', () => {
  let state = navigate(initialState(), 1);
  state.data = { orders: [order] };
  assert.ok(paletteCommands('', state).some((c) => c.key === 'S'));
  state = key(state, 'S').state;
  assert.equal(state.input.phase, 'edit');
  state = key(state, 'TRACK').state;
  state = key(state, '', 'tab').state;
  state = key(state, '', 'backspace').state;
  state = key(state, '1').state;
  let result = key(state, '', 's', { ctrl: true });
  assert.equal(result.effect, 'preview-shipment');
  assert.deepEqual(result.state.input.params, params);
  state = {
    ...result.state,
    input: {
      ...result.state.input,
      phase: 'review',
      busy: false,
      preview: {
        order,
        params,
        context,
        receipt: receipt('previewed', 'key'),
        applyAllowed: true,
        principalId: 'user:tui',
        storeId: 'store:tui',
      },
    },
  };
  state = key(state, '', 'return').state;
  assert.equal(state.input.phase, 'confirm');
  state = key(state, 'apply').state;
  result = key(state, '', 'return');
  assert.equal(result.effect, undefined);
  assert.match(result.state.input.error, /APPLY exactly/);
  state = key(result.state, '', 'escape').state;
  state = key(state, '', 'return').state;
  state = key(state, 'APPL').state;
  for (const [width, height] of [
    [20, 8],
    [80, 24],
    [140, 44],
  ]) {
    const frame = render(state, width, height, { color: false });
    assert.equal(state.input.text, 'APPL');
    assert.ok(frame.split('\n').length <= height);
  }
  state = key(state, 'Y').state;
  result = key(state, '', 'return');
  assert.equal(result.effect, 'apply-shipment');
  assert.equal(key(result.state, '', 'return').effect, undefined);
  assert.equal(key(result.state, 'q').effect, undefined);
});
test('shipment shortcuts recover separately and mismatched or unknown receipts never advertise normal success', () => {
  const state = navigate(initialState(), 1);
  assert.equal(key(state, 'G').effect, 'reconcile-shipment');
  assert.equal(key(state, 'J').effect, 'shipment-history');
  const lines = shipmentWorkflowLines(
    { phase: 'attention', receipt: receipt('succeeded', 'key'), reviewIssues: ['Wrong quantity'] },
    80,
  ).join('\n');
  assert.match(lines, /RECEIPT MISMATCH/);
  assert.doesNotMatch(lines, /RESERVED STOCK SHIPPED/);
  const unknown = {
    ...state,
    input: { kind: 'shipment-action', phase: 'uncertain', preview: { ticketId: 'host-ticket' } },
  };
  assert.equal(key(unknown, '', 'return').effect, 'apply-shipment');
  assert.equal(key(unknown, 'r').effect, undefined);
});
test('durable shipment intent reuses the original key after an uncertain apply and recovery never replays JSON', async (t) => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-shipment-unit-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const calls = [];
  let attempts = 0;
  const options = {
    dbPath: path.join(directory, 'owned.db'),
    directory,
    allowApply: true,
    loadKernel: shipmentKernel,
    invoke: async (operation, payload) => {
      calls.push({ operation, payload: structuredClone(payload) });
      if (operation === 'shipment-preview')
        return { order, params, context, receipt: receipt('previewed', payload.idempotencyKey) };
      if (operation === 'shipment-apply' && ++attempts === 1) throw new Error('connection lost');
      return { receipt: receipt('succeeded', payload.idempotencyKey) };
    },
  };
  const operations = new ShipmentOperations(options),
    preview = await operations.preview(params);
  await assert.rejects(
    operations.apply({ ticketId: preview.ticketId }),
    (error) => error.outcomeUnknown === true,
  );
  const saved = JSON.parse(
    await fs.readFile(path.join(directory, `shipment-action-${preview.ticketId}.json`)),
  );
  assert.equal(saved.state, 'pending');
  assert.deepEqual(saved.params, params);
  const result = await operations.apply({ ticketId: preview.ticketId });
  assert.deepEqual(result.reviewIssues, []);
  assert.equal(calls[1].payload.idempotencyKey, calls[2].payload.idempotencyKey);
  assert.equal((await operations.apply({ ticketId: preview.ticketId })).replayed, true);
  const restarted = new ShipmentOperations({ ...options, allowApply: false });
  assert.equal(
    (await restarted.reconcile({ historyId: preview.ticketId })).receipt.command_id,
    lineId,
  );
  assert.equal(calls.at(-1).operation, 'shipment-reconcile');
  assert.equal((await restarted.history({})).entries[0].quantity, '1');
  const retained = await operations.preview({ ...params, trackingNumber: '' });
  await operations.apply({ ticketId: retained.ticketId });
  assert.equal(
    (await restarted.history({})).entries.find((entry) => entry.id === retained.ticketId)
      .trackingNumber,
    'TRACK',
  );
  await assert.rejects(restarted.apply({ ticketId: preview.ticketId }), /Preview only/);
});
test('malformed or oversized preview context cannot issue a shipment apply ticket', async () => {
  for (const broken of [
    { ...context, lines: [null] },
    { ...context, lines: [] },
    { ...context, lines: Array.from({ length: 101 }, () => context.lines[0]) },
  ]) {
    const operations = new ShipmentOperations({
      dbPath: '/tmp/owned-shipment.db',
      allowApply: true,
      loadKernel: shipmentKernel,
      invoke: async (operation, payload) => ({
        order,
        params,
        context: broken,
        receipt: receipt('previewed', payload.idempotencyKey),
      }),
    });
    await assert.rejects(operations.preview(params), /review context/);
    assert.equal(operations.tickets.size, 0);
  }
});
test('the active quantity remains near the top of a large shipment editor', () => {
  let state = navigate(initialState(), 1);
  state.data = {
    orders: [
      {
        ...order,
        items: Array.from({ length: 100 }, (_, i) => ({ ...order.items[0], sku: `SKU-${i}` })),
      },
    ],
  };
  state = key(state, 'S').state;
  for (let i = 0; i < 100; i++) state = key(state, '', 'tab').state;
  assert.equal(state.input.field, 100);
  const lines = shipmentWorkflowLines(state.input, 78);
  assert.ok(lines.slice(0, 7).some((line) => line.includes('SKU-99')));
});
