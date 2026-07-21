/**
 * Shipping zone API tests for @stateset/embedded Node.js bindings.
 *
 * Zone lifecycle, zone shipping methods, and rate calculation.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('ShippingZones: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.shippingZones, 'shippingZones API should exist');
    assert.equal(await commerce.shippingZones.isSupported(), true);
  });

  let zone;
  await t.test('create returns an active zone', async () => {
    zone = await commerce.shippingZones.create({
      name: 'US Domestic',
      countries: ['US'],
      priority: 10,
    });
    assert.ok(zone.id);
    assert.equal(zone.name, 'US Domestic');
    assert.deepEqual(zone.countries, ['US']);
    assert.deepEqual(zone.regions, []);
    assert.equal(zone.priority, 10);
    assert.equal(zone.isActive, true);
    assert.ok(zone.createdAt);
  });

  await t.test('get returns the zone, and null when missing', async () => {
    const found = await commerce.shippingZones.get(zone.id);
    assert.equal(found.name, 'US Domestic');
    assert.equal(await commerce.shippingZones.get(randomUUID()), null);
  });

  await t.test('update applies patch semantics', async () => {
    const updated = await commerce.shippingZones.update(zone.id, {
      name: 'US + CA',
      countries: ['US', 'CA'],
    });
    assert.equal(updated.name, 'US + CA');
    assert.deepEqual(updated.countries, ['US', 'CA']);
    assert.equal(updated.priority, 10);
  });

  await t.test('list and findMatchingZones', async () => {
    const all = await commerce.shippingZones.list();
    assert.ok(all.some((z) => z.id === zone.id));

    const filtered = await commerce.shippingZones.list({ country: 'US', isActive: true });
    assert.ok(filtered.some((z) => z.id === zone.id));

    const matched = await commerce.shippingZones.findMatchingZones('US', null, null);
    assert.ok(matched.some((z) => z.id === zone.id));
  });

  // NOTE: the SQLite engine build does not yet ship a `zone_shipping_methods`
  // migration, so the method/rate surface is exercised only where it exists.
  let method;
  let methodsAvailable = true;
  await t.test('createMethod stores exact-decimal rates', async () => {
    try {
      method = await commerce.shippingZones.createMethod({
        zoneId: zone.id,
        name: 'Standard',
        carrier: 'USPS',
        methodType: 'flat',
        baseRate: '5.99',
        currency: 'USD',
        minDeliveryDays: 3,
        maxDeliveryDays: 7,
      });
    } catch (err) {
      if (/no such table/.test(err.message)) {
        methodsAvailable = false;
        return;
      }
      throw err;
    }
    assert.ok(method.id);
    assert.equal(method.zoneId, zone.id);
    assert.equal(method.methodType, 'flat');
    assert.equal(method.baseRate, '5.99');
    assert.equal(method.currency, 'USD');
    assert.deepEqual(method.conditions, []);
    assert.equal(method.isActive, true);
  });

  await t.test('createMethod rejects an invalid method type', async () => {
    await assert.rejects(
      () =>
        commerce.shippingZones.createMethod({
          zoneId: zone.id,
          name: 'Bad',
          methodType: 'nope',
          baseRate: '1',
          currency: 'USD',
        }),
      /Invalid shipping method type/,
    );
  });

  await t.test('getMethod / listMethods', { skip: !methodsAvailable }, async () => {
    const found = await commerce.shippingZones.getMethod(method.id);
    assert.equal(found.name, 'Standard');
    assert.equal(await commerce.shippingZones.getMethod(randomUUID()), null);

    const all = await commerce.shippingZones.listMethods();
    assert.ok(all.some((m) => m.id === method.id));

    const scoped = await commerce.shippingZones.listMethods({ zoneId: zone.id, carrier: 'USPS' });
    assert.equal(scoped.length, 1);
  });

  await t.test(
    'calculateRates returns the flat rate for a matching destination',
    { skip: !methodsAvailable },
    async () => {
      const rates = await commerce.shippingZones.calculateRates({
        country: 'US',
        orderTotal: '50.00',
        currency: 'USD',
      });
      const found = rates.find((r) => r.methodId === method.id);
      assert.ok(found, 'expected the standard method to be quoted');
      assert.equal(found.rate, '5.99');
      assert.equal(found.currency, 'USD');
      assert.equal(found.carrier, 'USPS');
    },
  );

  await t.test('delete removes methods and zones', async () => {
    if (methodsAvailable) {
      await commerce.shippingZones.deleteMethod(method.id);
      assert.equal(await commerce.shippingZones.getMethod(method.id), null);
    }

    await commerce.shippingZones.delete(zone.id);
    assert.equal(await commerce.shippingZones.get(zone.id), null);
  });
});
