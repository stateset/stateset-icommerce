/**
 * Purgatory (order ingestion staging) tests for @stateset/embedded Node.js bindings.
 *
 * Ingest, get, list, map line, post, delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('Purgatory: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.purgatory, 'purgatory API should exist');
    assert.equal(await commerce.purgatory.isSupported(), true);
  });

  let order;
  await t.test('ingest stages an order with unresolved lines', async () => {
    order = await commerce.purgatory.ingest({
      externalOrderId: 'EXT-1001',
      externalStatus: 'open',
      metadata: JSON.stringify({ source: 'edi' }),
      items: [
        { externalSku: 'EXT-SKU-1', quantity: '2' },
        { externalSku: 'EXT-SKU-2', quantity: '1.5', productId: randomUUID() },
      ],
    });
    assert.ok(order.id);
    assert.equal(order.isPosted, false);
    assert.equal(order.items.length, 2);
    assert.equal(order.items[0].quantity, '2');
    assert.equal(order.unresolvedCount, '1');
    assert.equal(order.isReadyToPost, false);
    assert.deepEqual(JSON.parse(order.metadata), { source: 'edi' });
  });

  await t.test('ingest rejects malformed metadata JSON', async () => {
    await assert.rejects(
      () =>
        commerce.purgatory.ingest({
          externalOrderId: 'EXT-BAD',
          metadata: '{not json',
          items: [],
        }),
      /Invalid metadata JSON/,
    );
  });

  await t.test('get returns the order, and null when missing', async () => {
    const found = await commerce.purgatory.get(order.id);
    assert.equal(found.id, order.id);
    assert.equal(await commerce.purgatory.get(randomUUID()), null);
  });

  await t.test('mapLine resolves the remaining line', async () => {
    const unresolved = order.items.find((i) => !i.isResolved);
    const updated = await commerce.purgatory.mapLine(order.id, unresolved.id, {
      productId: randomUUID(),
    });
    assert.equal(updated.unresolvedCount, '0');
    assert.equal(updated.isReadyToPost, true);
  });

  await t.test('list filters on posted state and paginates', async () => {
    assert.equal((await commerce.purgatory.list({ isPosted: false })).length, 1);
    assert.equal((await commerce.purgatory.list({ limit: 1, offset: 0 })).length, 1);
    assert.equal((await commerce.purgatory.list({ isPosted: true })).length, 0);
  });

  await t.test('post marks the order posted', async () => {
    const posted = await commerce.purgatory.post(order.id);
    assert.equal(posted.isPosted, true);
  });

  await t.test('delete removes the order', async () => {
    await commerce.purgatory.delete(order.id);
    assert.equal(await commerce.purgatory.get(order.id), null);
  });
});
