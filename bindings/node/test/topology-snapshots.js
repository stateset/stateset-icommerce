/**
 * Topology snapshot API tests for @stateset/embedded Node.js bindings.
 *
 * Capture, get, latest, list, delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('TopologySnapshots: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.topologySnapshots, 'topologySnapshots API should exist');
    assert.equal(await commerce.topologySnapshots.isSupported(), true);
  });

  let snapshot;
  await t.test('capture derives a health grade', async () => {
    snapshot = await commerce.topologySnapshots.capture({
      channelsTotal: '4',
      channelsActive: '4',
      warehousesTotal: '2',
      productsTotal: '120',
      openOrders: '17',
      signals: JSON.stringify({ lagSeconds: 3 }),
    });
    assert.ok(snapshot.id);
    assert.equal(snapshot.channelsTotal, '4');
    assert.equal(snapshot.openOrders, '17');
    assert.ok(['unknown', 'healthy', 'degraded', 'critical'].includes(snapshot.health));
    assert.deepEqual(JSON.parse(snapshot.signals), { lagSeconds: 3 });
    assert.ok(snapshot.capturedAt);
  });

  await t.test('capture rejects a non-numeric count', async () => {
    await assert.rejects(
      () =>
        commerce.topologySnapshots.capture({
          channelsTotal: 'many',
          channelsActive: '0',
          warehousesTotal: '0',
          productsTotal: '0',
          openOrders: '0',
        }),
      /Invalid channels_total count/,
    );
  });

  await t.test('get returns the snapshot, and null when missing', async () => {
    const found = await commerce.topologySnapshots.get(snapshot.id);
    assert.equal(found.id, snapshot.id);
    assert.equal(await commerce.topologySnapshots.get(randomUUID()), null);
  });

  await t.test('latest returns the most recent snapshot', async () => {
    const latest = await commerce.topologySnapshots.latest();
    assert.equal(latest.id, snapshot.id);
  });

  await t.test('list filters by health and paginates', async () => {
    assert.ok((await commerce.topologySnapshots.list()).length >= 1);
    assert.equal((await commerce.topologySnapshots.list({ limit: 1, offset: 0 })).length, 1);
    assert.equal(
      (await commerce.topologySnapshots.list({ health: snapshot.health })).length,
      1,
    );
    await assert.rejects(
      () => commerce.topologySnapshots.list({ health: 'fine' }),
      /Invalid health grade: fine/,
    );
  });

  await t.test('delete removes the snapshot', async () => {
    await commerce.topologySnapshots.delete(snapshot.id);
    assert.equal(await commerce.topologySnapshots.get(snapshot.id), null);
  });
});
