/**
 * Stock snapshot API tests for @stateset/embedded Node.js bindings.
 *
 * Capture, get, latest, list, delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('StockSnapshots: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.stockSnapshots, 'stockSnapshots API should exist');
    assert.equal(await commerce.stockSnapshots.isSupported(), true);
  });

  let snapshot;
  await t.test('capture computes totals from lines', async () => {
    snapshot = await commerce.stockSnapshots.capture({
      label: 'EOM',
      lines: [
        {
          productId: randomUUID(),
          sku: 'SKU-1',
          quantityOnHand: '10.5',
          quantityAvailable: '8',
          location: 'MAIN',
        },
        {
          productId: randomUUID(),
          sku: 'SKU-2',
          quantityOnHand: '5',
          quantityAvailable: '5',
        },
      ],
    });
    assert.ok(snapshot.id);
    assert.equal(snapshot.label, 'EOM');
    assert.equal(snapshot.totalSkus, '2');
    assert.equal(snapshot.totalUnits, '15.5');
    assert.equal(snapshot.lines.length, 2);
    assert.equal(snapshot.lines[0].quantityAvailable, '8');
    assert.equal(snapshot.lines[1].location, undefined);
    assert.ok(snapshot.capturedAt);
  });

  await t.test('capture rejects a bad decimal quantity', async () => {
    await assert.rejects(
      () =>
        commerce.stockSnapshots.capture({
          lines: [
            {
              productId: randomUUID(),
              sku: 'SKU-X',
              quantityOnHand: 'abc',
              quantityAvailable: '1',
            },
          ],
        }),
      /Invalid quantity_on_hand decimal/,
    );
  });

  await t.test('get returns the snapshot, and null when missing', async () => {
    const found = await commerce.stockSnapshots.get(snapshot.id);
    assert.equal(found.id, snapshot.id);
    assert.equal(await commerce.stockSnapshots.get(randomUUID()), null);
  });

  await t.test('latest returns the most recent snapshot', async () => {
    const latest = await commerce.stockSnapshots.latest();
    assert.equal(latest.id, snapshot.id);
  });

  await t.test('list supports no filter and pagination', async () => {
    const all = await commerce.stockSnapshots.list();
    assert.ok(all.length >= 1);
    const paged = await commerce.stockSnapshots.list({ limit: 1, offset: 0 });
    assert.equal(paged.length, 1);
  });

  await t.test('delete removes the snapshot', async () => {
    await commerce.stockSnapshots.delete(snapshot.id);
    assert.equal(await commerce.stockSnapshots.get(snapshot.id), null);
  });
});
