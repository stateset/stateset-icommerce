/**
 * Print station API tests for @stateset/embedded Node.js bindings.
 *
 * Pair, list, enqueue, pick up, complete, revoke.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('PrintStations: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.printStations, 'printStations API should exist');
    assert.equal(await commerce.printStations.isSupported(), true);
  });

  let station;
  await t.test('pair returns a station plus a one-time token', async () => {
    const result = await commerce.printStations.pair({
      name: 'Packing Bench 1',
      printers: ['zebra-1'],
    });
    station = result.station;
    assert.ok(result.token);
    assert.ok(station.id);
    assert.equal(station.name, 'Packing Bench 1');
    assert.deepEqual(station.printers, ['zebra-1']);
    assert.equal(station.revoked, false);
    assert.equal(station.lastSeenAt, undefined);
  });

  await t.test('listStations / getStation', async () => {
    const stations = await commerce.printStations.listStations();
    assert.ok(stations.some((s) => s.id === station.id));
    const found = await commerce.printStations.getStation(station.id);
    assert.equal(found.name, 'Packing Bench 1');
    assert.equal(await commerce.printStations.getStation(randomUUID()), null);
  });

  let job;
  await t.test('enqueueJob defaults the payload kind to zpl', async () => {
    job = await commerce.printStations.enqueueJob(station.id, {
      printerName: 'zebra-1',
      payload: '^XA^XZ',
    });
    assert.ok(job.id);
    assert.equal(job.stationId, station.id);
    assert.equal(job.payloadKind, 'zpl');
    assert.equal(job.status, 'queued');
    assert.equal(job.pickedUpAt, undefined);
  });

  await t.test('enqueueJob rejects an invalid payload kind', async () => {
    await assert.rejects(
      () =>
        commerce.printStations.enqueueJob(station.id, {
          payloadKind: 'nope',
          payload: 'x',
        }),
      /Invalid print payload kind/,
    );
  });

  await t.test('listJobs supports no filter and a status filter', async () => {
    const all = await commerce.printStations.listJobs(station.id);
    assert.equal(all.length, 1);
    const queued = await commerce.printStations.listJobs(station.id, { status: 'queued' });
    assert.equal(queued.length, 1);
    const printed = await commerce.printStations.listJobs(station.id, { status: 'printed' });
    assert.equal(printed.length, 0);
  });

  await t.test('nextJob picks up the queued job', async () => {
    const next = await commerce.printStations.nextJob(station.id);
    assert.equal(next.id, job.id);
    assert.equal(next.status, 'picked_up');
    assert.ok(next.pickedUpAt);
    assert.equal(await commerce.printStations.nextJob(station.id), null);
  });

  await t.test('completeJob marks the job printed', async () => {
    const done = await commerce.printStations.completeJob(job.id, true);
    assert.equal(done.status, 'printed');
  });

  await t.test('revokeStation flags the station revoked', async () => {
    const revoked = await commerce.printStations.revokeStation(station.id);
    assert.equal(revoked.revoked, true);
  });
});
