import assert from 'node:assert/strict';
import test from 'node:test';
import { reserveInventory } from '../src/state.mjs';

test('duplicate lines cannot oversell and failure leaves every SKU unchanged', () => {
  assert.equal(
    reserveInventory('oversell', [
      { sku: 'WIDGET-001', quantity: 30 },
      { sku: 'WIDGET-001', quantity: 30 },
      { sku: 'WIDGET-003', quantity: 1 },
    ]),
    null,
  );
  const result = reserveInventory('all-stock', [
    { sku: 'WIDGET-001', quantity: 47 },
    { sku: 'WIDGET-003', quantity: 12 },
  ]);
  assert.deepEqual(
    result.items.map((item) => item.available_after),
    [0, 0],
  );
});

test('equivalent replay is stock neutral; changed replay is rejected', () => {
  const first = reserveInventory('replay', [
    { sku: 'GADGET-A', quantity: 10 },
    { sku: 'GADGET-A', quantity: 15 },
  ]);
  assert.deepEqual(reserveInventory('replay', [{ sku: 'GADGET-A', quantity: 25 }]), first);
  assert.equal(reserveInventory('replay', [{ sku: 'GADGET-A', quantity: 26 }]), null);
  assert.equal(
    reserveInventory('remaining', [{ sku: 'GADGET-A', quantity: 175 }]).items[0].available_after,
    0,
  );
});

test('empty, malformed, negative, fractional, unknown and overflowing lines fail closed', () => {
  for (const lines of [
    [],
    null,
    [null],
    [{ sku: 'missing', quantity: 1 }],
    [{ sku: 'GADGET-B', quantity: -1 }],
    [{ sku: 'GADGET-B', quantity: 0.5 }],
    [
      { sku: 'GADGET-B', quantity: Number.MAX_SAFE_INTEGER },
      { sku: 'GADGET-B', quantity: 1 },
    ],
  ]) {
    assert.equal(reserveInventory('invalid', lines), null);
  }
  assert.equal(
    reserveInventory('untouched', [{ sku: 'GADGET-B', quantity: 100 }]).items[0].available_after,
    0,
  );
});
