/**
 * Serial numbers API tests for @stateset/embedded Node.js bindings.
 *
 * A serial walks a state machine (Available -> Sold, Available ->
 * Quarantined, ...). The binding exposes create, lookup, availability,
 * markSold and quarantine; a refused transition must leave the record exactly
 * as it was. Every test opens its own `:memory:` store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

async function customer(commerce, email = 'owner@example.com') {
  return commerce.customers.create({ email, firstName: 'Serial', lastName: 'Owner' });
}

test('create returns an available serial; get, getBySerial, list and count find it', async () => {
  const commerce = new Commerce(':memory:');
  const serial = await commerce.serials.create({ serial: 'SN-0001', sku: 'LAPTOP', manufacturedAt: '2026-05-01T00:00:00Z' });
  assert.ok(serial.id);
  assert.equal(serial.serial, 'SN-0001');
  assert.equal(serial.sku, 'LAPTOP');
  assert.equal(serial.status, 'Available');
  assert.equal(serial.ownerId, undefined);
  assert.equal(serial.locationId, undefined);
  assert.ok(serial.createdAt);

  assert.equal((await commerce.serials.get(serial.id)).id, serial.id);
  assert.equal((await commerce.serials.getBySerial('SN-0001')).id, serial.id);
  assert.deepEqual((await commerce.serials.list()).map((s) => s.id), [serial.id]);
  assert.equal(await commerce.serials.count(), 1);
  assert.equal(await commerce.serials.get('00000000-0000-0000-0000-000000000001'), null);
  assert.equal(await commerce.serials.getBySerial('NOPE'), null);
});

test('a serial string is generated when omitted and must be unique', async () => {
  const commerce = new Commerce(':memory:');
  const generated = await commerce.serials.create({ sku: 'LAPTOP' });
  assert.match(generated.serial, /^SN-/);
  assert.equal((await commerce.serials.getBySerial(generated.serial)).id, generated.id);

  await commerce.serials.create({ serial: 'SN-DUP', sku: 'LAPTOP' });
  await assert.rejects(commerce.serials.create({ serial: 'SN-DUP', sku: 'LAPTOP' }), (err) => err.code === 'CONFLICT');
  assert.equal(await commerce.serials.count(), 2);
});

test('isAvailable and getAvailable report only available serials for the SKU, honouring the limit', async () => {
  const commerce = new Commerce(':memory:');
  const first = await commerce.serials.create({ serial: 'SN-A', sku: 'LAPTOP' });
  const second = await commerce.serials.create({ serial: 'SN-B', sku: 'LAPTOP' });
  await commerce.serials.create({ serial: 'SN-C', sku: 'TABLET' });

  assert.equal(await commerce.serials.isAvailable('SN-A'), true);
  assert.equal(await commerce.serials.isAvailable('NOPE'), false);
  assert.deepEqual((await commerce.serials.getAvailable('LAPTOP', 10)).map((s) => s.id), [first.id, second.id]);
  assert.deepEqual((await commerce.serials.getAvailable('LAPTOP', 1)).map((s) => s.id), [first.id], 'oldest first, one only');
  assert.deepEqual(await commerce.serials.getAvailable('PHONE', 10), []);
});

test('markSold assigns the serial to the customer and takes it out of stock', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const serial = await commerce.serials.create({ serial: 'SN-SOLD', sku: 'LAPTOP' });
  const orderId = '00000000-0000-0000-0000-00000000aa01';

  const sold = await commerce.serials.markSold(serial.id, buyer.id, orderId);
  assert.equal(sold.id, serial.id);
  assert.equal(sold.status, 'Sold');
  assert.equal(sold.ownerId, buyer.id);
  assert.equal(await commerce.serials.isAvailable('SN-SOLD'), false);
  assert.deepEqual(await commerce.serials.getAvailable('LAPTOP', 10), []);
  assert.equal((await commerce.serials.getBySerial('SN-SOLD')).ownerId, buyer.id, 'ownership is traceable by serial');

  const withoutOrder = await commerce.serials.create({ serial: 'SN-GIFT', sku: 'LAPTOP' });
  assert.equal((await commerce.serials.markSold(withoutOrder.id, buyer.id)).status, 'Sold');
});

test('a sold serial cannot be sold or quarantined again; the refusal leaves it untouched', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const another = await customer(commerce, 'another@example.com');
  const serial = await commerce.serials.create({ serial: 'SN-ONCE', sku: 'LAPTOP' });
  await commerce.serials.markSold(serial.id, buyer.id);

  await assert.rejects(commerce.serials.markSold(serial.id, another.id), (err) => /sold to sold/i.test(err.message));
  await assert.rejects(commerce.serials.quarantine(serial.id, 'x'), (err) => /sold to quarantined/i.test(err.message));

  const after = await commerce.serials.get(serial.id);
  assert.equal(after.status, 'Sold');
  assert.equal(after.ownerId, buyer.id, 'the original owner keeps the unit');
});

test('quarantine takes an available serial out of stock and blocks a sale', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const serial = await commerce.serials.create({ serial: 'SN-BAD', sku: 'LAPTOP' });
  const good = await commerce.serials.create({ serial: 'SN-GOOD', sku: 'LAPTOP' });

  const quarantined = await commerce.serials.quarantine(serial.id, 'failed burn-in');
  assert.equal(quarantined.status, 'Quarantined');
  assert.equal(quarantined.ownerId, undefined);
  assert.equal(await commerce.serials.isAvailable('SN-BAD'), false);
  assert.deepEqual((await commerce.serials.getAvailable('LAPTOP', 10)).map((s) => s.id), [good.id]);

  await assert.rejects(commerce.serials.markSold(serial.id, buyer.id), (err) => /quarantined to sold/i.test(err.message));
  assert.equal((await commerce.serials.get(serial.id)).status, 'Quarantined');
  assert.equal(await commerce.serials.count(), 2);
});

test('malformed ids are VALIDATION and unknown ids are NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const serial = await commerce.serials.create({ serial: 'SN-IDS', sku: 'LAPTOP' });
  const unknown = '00000000-0000-0000-0000-000000000001';
  const isValidation = (pattern) => (err) => err.code === 'VALIDATION' && pattern.test(err.message);

  await assert.rejects(commerce.serials.get('not-a-uuid'), isValidation(/uuid/i));
  await assert.rejects(commerce.serials.quarantine('not-a-uuid', 'x'), isValidation(/uuid/i));
  await assert.rejects(commerce.serials.markSold('not-a-uuid', buyer.id), isValidation(/serial/i));
  await assert.rejects(commerce.serials.markSold(serial.id, 'not-a-uuid'), isValidation(/customer/i));
  await assert.rejects(commerce.serials.markSold(serial.id, buyer.id, 'not-a-uuid'), isValidation(/order/i));
  await assert.rejects(commerce.serials.markSold(unknown, buyer.id), (err) => err.code === 'NOT_FOUND');
  await assert.rejects(commerce.serials.quarantine(unknown, 'x'), (err) => err.code === 'NOT_FOUND');

  assert.equal((await commerce.serials.get(serial.id)).status, 'Available', 'a refused call writes nothing');
  assert.equal(await commerce.serials.count(), 1);
});

test(
  'a refused state transition is PRECONDITION_FAILED',
  { todo: 'engine: SerialNumber::ensure_can_transition_to raises CommerceError::Conflict, which the binding reports as CONFLICT; the record is in the wrong state, not in a race' },
  async () => {
    const commerce = new Commerce(':memory:');
    const buyer = await customer(commerce);
    const serial = await commerce.serials.create({ serial: 'SN-STATE', sku: 'LAPTOP' });
    await commerce.serials.markSold(serial.id, buyer.id);
    await assert.rejects(commerce.serials.markSold(serial.id, buyer.id), (err) => err.code === 'PRECONDITION_FAILED');
    await assert.rejects(commerce.serials.quarantine(serial.id, 'x'), (err) => err.code === 'PRECONDITION_FAILED');
  },
);

test(
  'a malformed manufacturedAt is refused instead of dropped',
  async () => {
    const commerce = new Commerce(':memory:');
    await assert.rejects(
      commerce.serials.create({ sku: 'LAPTOP', manufacturedAt: 'yesterday' }),
      (err) => err.code === 'VALIDATION' && /manufactured/i.test(err.message),
    );
    assert.equal(await commerce.serials.count(), 0);
  },
);

test(
  'a serial created with a lotNumber is linked to that lot',
  async () => {
    const commerce = new Commerce(':memory:');
    const lot = await commerce.lots.create({ lotNumber: 'LOT-LINK', sku: 'LAPTOP', quantityProduced: 1 });
    const serial = await commerce.serials.create({ serial: 'SN-LINK', sku: 'LAPTOP', lotNumber: 'LOT-LINK' });
    assert.equal(serial.lotId, lot.id);
    assert.equal((await commerce.serials.getBySerial('SN-LINK')).lotId, lot.id);
  },
);
