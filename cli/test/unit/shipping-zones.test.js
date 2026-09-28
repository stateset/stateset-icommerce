/**
 * Shipping zone tool handlers against a recording mock of the binding's
 * `ShippingZones` class. The mock defines only methods the binding declares
 * (checked below), and every test asserts the exact call the handler makes.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { shippingZoneTools } from '../../src/tools/shipping-zones.js';
import { bindingClassMethods } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(shippingZoneTools.map((t) => [t.name, t]));

/** A ShippingZoneOutput, exactly as the binding shapes it. */
function zone(overrides = {}) {
  return {
    id: 'zone-1',
    name: 'Domestic',
    countries: ['US'],
    regions: ['CA', 'NY'],
    postalCodes: [],
    priority: 1,
    isActive: true,
    createdAt: '2026-09-27T00:00:00+00:00',
    updatedAt: '2026-09-27T00:00:00+00:00',
    ...overrides,
  };
}

/** A ZoneShippingMethodOutput. */
function method(overrides = {}) {
  return {
    id: 'm-1',
    zoneId: 'zone-1',
    name: 'Ground',
    carrier: 'UPS',
    methodType: 'flat',
    baseRate: '5.99',
    currency: 'USD',
    minDeliveryDays: 3,
    maxDeliveryDays: 5,
    conditions: [],
    isActive: true,
    createdAt: '2026-09-27T00:00:00+00:00',
    updatedAt: '2026-09-27T00:00:00+00:00',
    ...overrides,
  };
}

/** Recording mock: `{ commerce, calls }`, with `calls` as `[method, ...args]`. */
function makeCommerce(impls = {}) {
  const calls = [];
  const defaults = {
    create: async (input) => zone(input),
    get: async () => zone(),
    update: async (_id, input) => zone(input),
    list: async () => [zone()],
    createMethod: async (input) => method(input),
    listMethods: async () => [method()],
    calculateRates: async () => [
      {
        methodId: 'm-1',
        methodName: 'Ground',
        carrier: 'UPS',
        rate: '5.99',
        currency: 'USD',
        minDeliveryDays: 3,
        maxDeliveryDays: 5,
      },
    ],
  };
  const shippingZones = {};
  for (const [name, fn] of Object.entries({ ...defaults, ...impls })) {
    shippingZones[name] = async (...args) => {
      calls.push([name, ...args]);
      return fn(...args);
    };
  }
  return { commerce: { shippingZones }, calls };
}

describe('shipping zone mock', () => {
  it('defines only methods the binding’s ShippingZones class declares', () => {
    const real = bindingClassMethods('ShippingZones');
    for (const name of Object.keys(makeCommerce().commerce.shippingZones)) {
      assert.ok(real.has(name), `mock invents shippingZones.${name}`);
    }
  });
});

describe('create_shipping_zone / update_shipping_zone', () => {
  it('create sends CreateShippingZoneInput', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.create_shipping_zone.handler({
      commerce,
      params: { name: 'Domestic', countries: ['US'], regions: ['CA'], priority: 1 },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      [
        'create',
        {
          name: 'Domestic',
          countries: ['US'],
          regions: ['CA'],
          postalCodes: undefined,
          priority: 1,
        },
      ],
    ]);
  });

  it('update sends (id, UpdateShippingZoneInput)', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.update_shipping_zone.handler({
      commerce,
      params: { zoneId: 'zone-1', name: 'Domestic US', isActive: false, postalCodes: ['9*'] },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      [
        'update',
        'zone-1',
        {
          name: 'Domestic US',
          countries: undefined,
          regions: undefined,
          postalCodes: ['9*'],
          priority: undefined,
          isActive: false,
        },
      ],
    ]);
  });
});

describe('get_shipping_zone', () => {
  it('returns the zone’s real fields plus its methods', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.get_shipping_zone.handler({
      commerce,
      params: { zoneId: 'zone-1' },
    });
    assert.deepEqual(calls, [
      ['get', 'zone-1'],
      ['listMethods', { zoneId: 'zone-1' }],
    ]);
    assert.equal(result.zone.isActive, true);
    assert.deepEqual(result.zone.postalCodes, []);
    assert.equal(result.zone.methods[0].baseRate, '5.99');
  });

  it('reports not found without listing methods', async () => {
    const { commerce, calls } = makeCommerce({ get: async () => null });
    const result = await byName.get_shipping_zone.handler({
      commerce,
      params: { zoneId: 'nope' },
    });
    assert.deepEqual(result, { success: false, error: 'Shipping zone not found' });
    assert.equal(calls.length, 1);
  });
});

describe('list_shipping_zones', () => {
  it('passes a real ShippingZoneFilterInput with an explicit page limit', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.list_shipping_zones.handler({
      commerce,
      params: { country: 'US', isActive: true, limit: 50 },
    });
    assert.deepEqual(calls, [['list', { country: 'US', isActive: true, limit: 500, offset: 0 }]]);
  });

  it('pages past the engine’s 500-row default, so totalCount is the real total', async () => {
    const zones = Array.from({ length: 1003 }, (_, i) => zone({ id: `zone-${i}` }));
    const { commerce, calls } = makeCommerce({
      list: async ({ limit, offset }) => zones.slice(offset, offset + limit),
    });
    const result = await byName.list_shipping_zones.handler({ commerce, params: { limit: 3 } });
    assert.equal(calls.length, 3);
    assert.equal(result.totalCount, 1003);
    assert.equal(result.returned, 3);
    assert.deepEqual(Object.keys(result.zones[0]).sort(), [
      'countries',
      'createdAt',
      'id',
      'isActive',
      'name',
      'postalCodes',
      'priority',
      'regions',
      'updatedAt',
    ]);
  });
});

describe('create_shipping_method', () => {
  it('sends CreateZoneShippingMethodInput with exact decimal strings', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.create_shipping_method.handler({
      commerce,
      params: {
        zoneId: 'zone-1',
        name: 'Std',
        methodType: 'price_based',
        baseRate: 5.99,
        conditions: [
          { minPrice: 75, rate: 0 },
          { minPrice: '0', maxPrice: '74.99', rate: '5.99' },
        ],
        currency: 'USD',
      },
      allowApply: true,
    });
    assert.deepEqual(calls, [
      [
        'createMethod',
        {
          zoneId: 'zone-1',
          name: 'Std',
          carrier: undefined,
          methodType: 'price_based',
          baseRate: '5.99',
          currency: 'USD',
          minDeliveryDays: undefined,
          maxDeliveryDays: undefined,
          conditions: [
            {
              minWeight: undefined,
              maxWeight: undefined,
              minPrice: '75',
              maxPrice: undefined,
              rate: '0',
            },
            {
              minWeight: undefined,
              maxWeight: undefined,
              minPrice: '0',
              maxPrice: '74.99',
              rate: '5.99',
            },
          ],
        },
      ],
    ]);
  });

  it('defaults to a flat USD method', async () => {
    const { commerce, calls } = makeCommerce();
    await byName.create_shipping_method.handler({
      commerce,
      params: { zoneId: 'zone-1', name: 'Ground', baseRate: 5 },
      allowApply: true,
    });
    assert.equal(calls[0][1].methodType, 'flat');
    assert.equal(calls[0][1].currency, 'USD');
    assert.equal(calls[0][1].baseRate, '5');
  });
});

describe('calculate_shipping_rate / list_shipping_methods', () => {
  it('calculateRates gets a ZoneShippingRateRequestInput', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.calculate_shipping_rate.handler({
      commerce,
      params: { country: 'US', region: 'CA', orderTotal: 100, weight: '2.5' },
    });
    assert.deepEqual(calls, [
      [
        'calculateRates',
        {
          country: 'US',
          region: 'CA',
          postalCode: undefined,
          weight: '2.5',
          orderTotal: '100',
          currency: 'USD',
        },
      ],
    ]);
    assert.equal(result.rates[0].rate, '5.99');
  });

  it('listMethods gets a ZoneShippingMethodFilterInput, not a bare id', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await byName.list_shipping_methods.handler({
      commerce,
      params: { zoneId: 'zone-1' },
    });
    assert.deepEqual(calls, [['listMethods', { zoneId: 'zone-1' }]]);
    assert.equal(result.count, 1);
    assert.equal(result.methods[0].methodType, 'flat');
  });
});
