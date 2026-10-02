/**
 * Lots (batch tracking) API tests for @stateset/embedded Node.js bindings.
 *
 * Quantities are asserted exactly (the engine keeps them as Decimal; the
 * binding hands back numbers that are exact for the fractions used here).
 * Dates are compared by instant, since the engine renders them with a
 * `+00:00` offset. Every test opens its own `:memory:` store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const DAY_MS = 86_400_000;
const daysFromNow = (days) => new Date(Date.now() + days * DAY_MS).toISOString();
const sameInstant = (actual, expected) => assert.equal(Date.parse(actual), Date.parse(expected));

test('create returns an active lot with everything available; get, getByNumber, list and count find it', async () => {
  const commerce = new Commerce(':memory:');
  const expires = '2026-12-31T00:00:00Z';
  const lot = await commerce.lots.create({
    lotNumber: 'LOT-2026-001',
    sku: 'WIDGET',
    quantityProduced: 100,
    productionDate: '2026-09-01T00:00:00Z',
    expirationDate: expires,
    supplierLotNumber: 'SUP-77',
  });
  assert.ok(lot.id);
  assert.equal(lot.lotNumber, 'LOT-2026-001');
  assert.equal(lot.sku, 'WIDGET');
  assert.equal(lot.status, 'Active');
  assert.equal(lot.quantityProduced, 100);
  assert.equal(lot.quantityAvailable, 100);
  assert.equal(lot.quantityReserved, 0);
  sameInstant(lot.productionDate, '2026-09-01T00:00:00Z');
  sameInstant(lot.expirationDate, expires);

  assert.equal((await commerce.lots.get(lot.id)).id, lot.id);
  assert.equal((await commerce.lots.getByNumber('LOT-2026-001')).id, lot.id);
  assert.deepEqual((await commerce.lots.list()).map((l) => l.id), [lot.id]);
  assert.equal(await commerce.lots.count(), 1);
  assert.equal(await commerce.lots.get('00000000-0000-0000-0000-000000000001'), null);
  assert.equal(await commerce.lots.getByNumber('NOPE'), null);
});

test('a lot number is generated when omitted and fractional quantities survive exactly', async () => {
  const commerce = new Commerce(':memory:');
  const lot = await commerce.lots.create({ sku: 'RESIN', quantityProduced: 40.5 });
  assert.match(lot.lotNumber, /^LOT-/);
  assert.equal(lot.quantityProduced, 40.5);
  assert.equal(lot.quantityAvailable, 40.5);
  assert.equal(lot.expirationDate, undefined);
  assert.ok(lot.productionDate, 'production date defaults to now');
  assert.equal((await commerce.lots.getByNumber(lot.lotNumber)).id, lot.id);
});

test('duplicate lot numbers are a CONFLICT', async () => {
  const commerce = new Commerce(':memory:');
  await commerce.lots.create({ lotNumber: 'LOT-DUP', sku: 'WIDGET', quantityProduced: 1 });
  await assert.rejects(
    commerce.lots.create({ lotNumber: 'LOT-DUP', sku: 'WIDGET', quantityProduced: 2 }),
    (err) => err.code === 'CONFLICT',
  );
  assert.equal(await commerce.lots.count(), 1);
});

test('quarantine blocks every unit and removes the lot from allocation', async () => {
  const commerce = new Commerce(':memory:');
  const lot = await commerce.lots.create({ lotNumber: 'LOT-Q', sku: 'WIDGET', quantityProduced: 100 });
  const other = await commerce.lots.create({ lotNumber: 'LOT-OK', sku: 'WIDGET', quantityProduced: 5 });

  const quarantined = await commerce.lots.quarantine(lot.id, 'contamination suspected');
  assert.equal(quarantined.status, 'Quarantine');
  assert.equal(quarantined.quantityAvailable, 0);
  assert.equal(quarantined.quantityProduced, 100, 'the produced count is history and does not move');
  assert.equal(quarantined.quantityReserved, 0);

  assert.deepEqual((await commerce.lots.getQuarantined()).map((l) => l.id), [lot.id]);
  assert.deepEqual((await commerce.lots.getAvailableLotsForSku('WIDGET')).map((l) => l.id), [other.id]);
  assert.deepEqual((await commerce.lots.getActiveLots('WIDGET')).map((l) => l.id), [other.id]);
  assert.equal((await commerce.lots.get(lot.id)).status, 'Quarantine');
});

test('a quarantined lot cannot be quarantined again; the refusal leaves it untouched', async () => {
  const commerce = new Commerce(':memory:');
  const lot = await commerce.lots.create({ lotNumber: 'LOT-Q2', sku: 'WIDGET', quantityProduced: 50 });
  await commerce.lots.quarantine(lot.id, 'first');
  await assert.rejects(
    commerce.lots.quarantine(lot.id, 'again'),
    (err) => err.code === 'VALIDATION' && /LOT-Q2/.test(err.message) && /quarantine/i.test(err.message),
  );
  const after = await commerce.lots.get(lot.id);
  assert.equal(after.status, 'Quarantine');
  assert.equal(after.quantityAvailable, 0);
  assert.deepEqual((await commerce.lots.getQuarantined()).map((l) => l.id), [lot.id]);
});

test('releaseQuarantine restores the units; only a quarantined lot can be released', async () => {
  const commerce = new Commerce(':memory:');
  const lot = await commerce.lots.create({ lotNumber: 'LOT-R', sku: 'WIDGET', quantityProduced: 100 });
  await assert.rejects(
    commerce.lots.releaseQuarantine(lot.id),
    (err) => err.code === 'VALIDATION' && /status is active/i.test(err.message),
  );

  await commerce.lots.quarantine(lot.id, 'hold for inspection');
  const released = await commerce.lots.releaseQuarantine(lot.id);
  assert.equal(released.status, 'Active');
  assert.equal(released.quantityAvailable, 100);
  assert.deepEqual(await commerce.lots.getQuarantined(), []);
  assert.deepEqual((await commerce.lots.getAvailableLotsForSku('WIDGET')).map((l) => l.id), [lot.id]);

  await assert.rejects(commerce.lots.releaseQuarantine(lot.id), (err) => err.code === 'VALIDATION' && /not quarantine/i.test(err.message));
});

test('getAvailableLotsForSku allocates first-expiry-first-out, with never-expiring lots last', async () => {
  const commerce = new Commerce(':memory:');
  const later = await commerce.lots.create({ lotNumber: 'LOT-LATER', sku: 'MILK', quantityProduced: 10, expirationDate: daysFromNow(10) });
  const never = await commerce.lots.create({ lotNumber: 'LOT-NEVER', sku: 'MILK', quantityProduced: 10 });
  const soon = await commerce.lots.create({ lotNumber: 'LOT-SOON', sku: 'MILK', quantityProduced: 10, expirationDate: daysFromNow(3) });
  await commerce.lots.create({ lotNumber: 'LOT-OTHER-SKU', sku: 'CREAM', quantityProduced: 10, expirationDate: daysFromNow(1) });

  assert.deepEqual(
    (await commerce.lots.getAvailableLotsForSku('MILK')).map((l) => l.lotNumber),
    ['LOT-SOON', 'LOT-LATER', 'LOT-NEVER'],
  );
  assert.deepEqual(new Set((await commerce.lots.getActiveLots('MILK')).map((l) => l.id)), new Set([later.id, never.id, soon.id]));
  assert.deepEqual(await commerce.lots.getAvailableLotsForSku('NOTHING'), []);
});

test('expiry queries: getExpiringLots honours its window and getExpiredLots separates the past', async () => {
  const commerce = new Commerce(':memory:');
  await commerce.lots.create({ lotNumber: 'LOT-3D', sku: 'MILK', quantityProduced: 1, expirationDate: daysFromNow(3) });
  await commerce.lots.create({ lotNumber: 'LOT-20D', sku: 'MILK', quantityProduced: 1, expirationDate: daysFromNow(20) });
  await commerce.lots.create({ lotNumber: 'LOT-PAST', sku: 'MILK', quantityProduced: 1, expirationDate: '2020-01-01T00:00:00Z' });
  await commerce.lots.create({ lotNumber: 'LOT-FOREVER', sku: 'MILK', quantityProduced: 1 });

  assert.deepEqual((await commerce.lots.getExpiringLots(5)).map((l) => l.lotNumber), ['LOT-3D']);
  assert.deepEqual((await commerce.lots.getExpiringLots(30)).map((l) => l.lotNumber), ['LOT-3D', 'LOT-20D']);
  assert.deepEqual((await commerce.lots.getExpiredLots()).map((l) => l.lotNumber), ['LOT-PAST']);
  assert.ok(!(await commerce.lots.getAvailableLotsForSku('MILK')).some((l) => l.lotNumber === 'LOT-PAST'), 'an expired lot is never allocated');
  assert.equal(await commerce.lots.count(), 4);
});

test('malformed ids are VALIDATION and unknown ids are NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  const unknown = '00000000-0000-0000-0000-000000000001';
  for (const call of [
    () => commerce.lots.get('not-a-uuid'),
    () => commerce.lots.quarantine('not-a-uuid', 'x'),
    () => commerce.lots.releaseQuarantine('not-a-uuid'),
  ]) {
    await assert.rejects(call, (err) => err.code === 'VALIDATION' && /uuid/i.test(err.message));
  }
  await assert.rejects(commerce.lots.quarantine(unknown, 'x'), (err) => err.code === 'NOT_FOUND');
  await assert.rejects(commerce.lots.releaseQuarantine(unknown), (err) => err.code === 'NOT_FOUND');
  assert.equal(await commerce.lots.count(), 0);
});

test(
  'a lot must be created with a positive quantity',
  async () => {
    const commerce = new Commerce(':memory:');
    const isValidation = (err) => err.code === 'VALIDATION' && /quantity/i.test(err.message);
    await assert.rejects(commerce.lots.create({ sku: 'WIDGET', quantityProduced: 0 }), isValidation);
    await assert.rejects(commerce.lots.create({ sku: 'WIDGET', quantityProduced: -5 }), isValidation);
    assert.equal(await commerce.lots.count(), 0);
  },
);

test(
  'a malformed production or expiration date is refused instead of dropped',
  async () => {
    const commerce = new Commerce(':memory:');
    const isValidation = (err) => err.code === 'VALIDATION' && /date/i.test(err.message);
    await assert.rejects(commerce.lots.create({ sku: 'WIDGET', quantityProduced: 1, expirationDate: 'next-week' }), isValidation);
    await assert.rejects(commerce.lots.create({ sku: 'WIDGET', quantityProduced: 1, productionDate: 'garbage' }), isValidation);
    assert.equal(await commerce.lots.count(), 0);
  },
);

test(
  'quarantining a lot cascades to the serials that belong to it',
  async () => {
    const commerce = new Commerce(':memory:');
    const lot = await commerce.lots.create({ lotNumber: 'LOT-CASCADE', sku: 'WIDGET', quantityProduced: 2 });
    const serial = await commerce.serials.create({ serial: 'SN-CASCADE-1', sku: 'WIDGET', lotNumber: 'LOT-CASCADE' });
    assert.equal(serial.lotId, lot.id);
    await commerce.lots.quarantine(lot.id, 'recall');
    assert.equal((await commerce.serials.getBySerial('SN-CASCADE-1')).status, 'Quarantined');
    assert.equal(await commerce.serials.isAvailable('SN-CASCADE-1'), false);
    await commerce.lots.releaseQuarantine(lot.id);
    assert.equal((await commerce.serials.getBySerial('SN-CASCADE-1')).status, 'Available');
  },
);
