/**
 * Shipping zone tool definitions: names, permissions, input schemas, the
 * --apply guard, and that the module only calls methods the binding's
 * `ShippingZones` class declares.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { shippingZoneTools } from '../../src/tools/shipping-zones.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(shippingZoneTools.map((t) => [t.name, t]));
const parse = (name, params) => z.object(byName[name].inputSchema).safeParse(params);

describe('shippingZoneTools -- module exports', () => {
  it('exports the expected tools in order', () => {
    assert.deepEqual(
      shippingZoneTools.map((t) => t.name),
      [
        'create_shipping_zone',
        'get_shipping_zone',
        'list_shipping_zones',
        'update_shipping_zone',
        'create_shipping_method',
        'calculate_shipping_rate',
        'list_shipping_methods',
      ],
    );
  });

  it('read and write permissions are unchanged', () => {
    for (const name of [
      'get_shipping_zone',
      'list_shipping_zones',
      'calculate_shipping_rate',
      'list_shipping_methods',
    ]) {
      assert.equal(byName[name].permission, 'read', name);
    }
    for (const name of ['create_shipping_zone', 'update_shipping_zone', 'create_shipping_method']) {
      assert.equal(byName[name].permission, 'write', name);
    }
  });
});

describe('shippingZoneTools -- binding surface', () => {
  it('calls only methods the binding’s ShippingZones class declares', () => {
    const methods = bindingClassMethods('ShippingZones');
    const called = toolModuleCalls('shipping-zones.js', 'shippingZones');
    assert.ok(called.size >= 6, `only found ${[...called]}`);
    for (const method of called) {
      assert.ok(methods.has(method), `commerce.shippingZones.${method} is not on the binding`);
    }
    assert.equal(called.has('count'), false);
  });
});

describe('shippingZoneTools -- input schemas', () => {
  it('zones take postalCodes and priority (CreateShippingZoneInput), not ranges', () => {
    assert.deepEqual(Object.keys(byName.create_shipping_zone.inputSchema).sort(), [
      'countries',
      'name',
      'postalCodes',
      'priority',
      'regions',
    ]);
    assert.ok(
      parse('create_shipping_zone', { name: 'US', countries: ['US'], postalCodes: ['90001'] })
        .success,
    );
    assert.equal(parse('create_shipping_zone', { name: 'US', countries: [] }).success, false);
  });

  it('update_shipping_zone can toggle isActive', () => {
    assert.ok(parse('update_shipping_zone', { zoneId: 'z', isActive: false }).success);
  });

  it('create_shipping_method takes a real method type and decimal conditions', () => {
    for (const methodType of ['flat', 'weight_based', 'price_based', 'calculated', 'free']) {
      assert.ok(
        parse('create_shipping_method', { zoneId: 'z', name: 'n', baseRate: 5, methodType })
          .success,
        methodType,
      );
    }
    assert.equal(
      parse('create_shipping_method', { zoneId: 'z', name: 'n', baseRate: 5, methodType: 'x' })
        .success,
      false,
    );
    assert.ok(
      parse('create_shipping_method', {
        zoneId: 'z',
        name: 'n',
        baseRate: '5.99',
        methodType: 'price_based',
        conditions: [
          { minPrice: 75, rate: 0 },
          { maxPrice: '74.99', rate: '5.99' },
        ],
      }).success,
    );
    assert.equal(
      parse('create_shipping_method', { zoneId: 'z', name: 'n', baseRate: '5,99' }).success,
      false,
    );
    assert.equal(parse('create_shipping_method', { zoneId: 'z', name: 'n' }).success, false);
  });

  it('calculate_shipping_rate takes weight / orderTotal (ZoneShippingRateRequestInput)', () => {
    assert.ok(parse('calculate_shipping_rate', { country: 'US' }).success);
    assert.ok(
      parse('calculate_shipping_rate', { country: 'US', weight: 2.5, orderTotal: '100.00' })
        .success,
    );
    assert.equal(parse('calculate_shipping_rate', { country: 'U' }).success, false);
  });

  it('list limit defaults to 50 and caps at 500', () => {
    assert.equal(parse('list_shipping_zones', {}).data.limit, 50);
    assert.equal(parse('list_shipping_zones', { limit: 501 }).success, false);
  });
});

describe('shippingZoneTools -- --apply guard', () => {
  const untouchable = new Proxy(
    {},
    {
      get() {
        throw new Error('the binding must not be reached without --apply');
      },
    },
  );

  for (const [name, params] of [
    ['create_shipping_zone', { name: 'US', countries: ['US'] }],
    ['update_shipping_zone', { zoneId: 'z', name: 'x' }],
    ['create_shipping_method', { zoneId: 'z', name: 'n', baseRate: 5 }],
  ]) {
    it(`${name} previews without --apply`, async () => {
      const result = await byName[name].handler({
        commerce: untouchable,
        params,
        allowApply: false,
      });
      assert.equal(result.success, false);
      assert.match(result.error, /--apply/);
      assert.deepEqual(result.wouldDo, params);
    });
  }
});
