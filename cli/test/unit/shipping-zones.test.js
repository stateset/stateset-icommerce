/**
 * Shipping Zone Tools Test Suite
 *
 * Tests for cli/src/tools/shipping-zones.js
 * Covers: create_shipping_zone, get_shipping_zone, list_shipping_zones,
 *         update_shipping_zone, create_shipping_method,
 *         calculate_shipping_rate, list_shipping_methods
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { shippingZoneTools } from '../../src/tools/shipping-zones.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockZone = {
  id: 'zone_001',
  name: 'US Continental',
  countries: ['US'],
  regions: [],
  postalCodes: [],
  postalCodeRanges: [],
  methodCount: 2,
  methods: [],
  status: 'active',
  createdAt: '2026-02-21T00:00:00Z',
  updatedAt: '2026-02-21T00:00:00Z',
};

const mockMethod = {
  id: 'method_001',
  zoneId: 'zone_001',
  name: 'Standard',
  carrier: 'USPS',
  baseRate: '9.99',
  perItemRate: '0.00',
  freeShippingThreshold: null,
  minDeliveryDays: 3,
  maxDeliveryDays: 7,
  currency: 'USD',
  status: 'active',
};

const mockRate = {
  methodId: 'method_001',
  methodName: 'Standard',
  carrier: 'USPS',
  rate: '9.99',
  currency: 'USD',
  minDeliveryDays: 3,
  maxDeliveryDays: 7,
  isFreeShipping: false,
};

function makeShippingZoneCommerce(overrides = {}) {
  return {
    shippingZones: {
      create: async (data) => ({ ...mockZone, ...data }),
      get: async (_id) => mockZone,
      list: async () => [mockZone],
      count: async () => 1,
      update: async (_id, data) => ({ ...mockZone, ...data }),
      createMethod: async (_zoneId, data) => ({ ...mockMethod, ...data }),
      calculateRates: async (_params) => [mockRate],
      listMethods: async (_zoneId) => [mockMethod],
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Shipping Zone Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(shippingZoneTools));
  });

  it('has at least 7 tools', () => {
    assert.ok(shippingZoneTools.length >= 7, `Expected >= 7, got ${shippingZoneTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of shippingZoneTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// create_shipping_zone
// ============================================================================

describe('create_shipping_zone', () => {
  const tool = findTool(shippingZoneTools, 'create_shipping_zone');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { name: 'US Continental', countries: ['US'] },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('creates zone with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { name: 'US Continental', countries: ['US'], regions: ['CA', 'NY'] },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.zone);
    assert.equal(result.zone.name, 'US Continental');
  });

  it('passes correct data to commerce.shippingZones.create()', async () => {
    let calledWith = null;
    const commerce = makeShippingZoneCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockZone, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { name: 'EU Zone', countries: ['DE', 'FR', 'IT'], regions: [] },
      allowApply: true,
    });
    assert.equal(calledWith.name, 'EU Zone');
    assert.deepEqual(calledWith.countries, ['DE', 'FR', 'IT']);
  });

  it('returns error when create throws', async () => {
    const commerce = makeShippingZoneCommerce({
      create: async () => {
        throw new Error('Duplicate zone name');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { name: 'US Continental', countries: ['US'] },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Duplicate zone name'));
    }
  });
});

// ============================================================================
// get_shipping_zone
// ============================================================================

describe('get_shipping_zone', () => {
  const tool = findTool(shippingZoneTools, 'get_shipping_zone');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns zone for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { zoneId: 'zone_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.zone.id, 'zone_001');
    assert.equal(result.zone.name, 'US Continental');
    assert.deepEqual(result.zone.countries, ['US']);
  });

  it('returns success: false when zone not found', async () => {
    const commerce = makeShippingZoneCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { zoneId: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeShippingZoneCommerce({
      get: async () => {
        throw new Error('DB lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { zoneId: 'zone_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB lookup failed'));
    }
  });
});

// ============================================================================
// list_shipping_zones
// ============================================================================

describe('list_shipping_zones', () => {
  const tool = findTool(shippingZoneTools, 'list_shipping_zones');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list with totalCount and returned', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.zones.length, 1);
    assert.equal(result.zones[0].id, 'zone_001');
    assert.equal(result.zones[0].name, 'US Continental');
  });

  it('maps expected fields on each zone', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: {},
    });
    const z = result.zones[0];
    assert.ok('id' in z);
    assert.ok('name' in z);
    assert.ok('countries' in z);
    assert.ok('methodCount' in z);
    assert.ok('status' in z);
    assert.ok('createdAt' in z);
  });

  it('returns error when list throws', async () => {
    const commerce = makeShippingZoneCommerce({
      list: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: {} });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// update_shipping_zone
// ============================================================================

describe('update_shipping_zone', () => {
  const tool = findTool(shippingZoneTools, 'update_shipping_zone');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { zoneId: 'zone_001', name: 'CONUS' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('updates zone with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { zoneId: 'zone_001', name: 'CONUS', countries: ['US'] },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('updated'));
    assert.ok(result.zone);
  });

  it('passes correct arguments to commerce.shippingZones.update()', async () => {
    let calledId = null;
    let calledData = null;
    const commerce = makeShippingZoneCommerce({
      update: async (id, data) => {
        calledId = id;
        calledData = data;
        return { ...mockZone, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { zoneId: 'zone_001', name: 'New Name', countries: ['US', 'CA'] },
      allowApply: true,
    });
    assert.equal(calledId, 'zone_001');
    assert.equal(calledData.name, 'New Name');
    assert.deepEqual(calledData.countries, ['US', 'CA']);
  });

  it('returns error when update throws', async () => {
    const commerce = makeShippingZoneCommerce({
      update: async () => {
        throw new Error('Zone not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { zoneId: 'bad_id', name: 'x' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Zone not found'));
    }
  });
});

// ============================================================================
// create_shipping_method
// ============================================================================

describe('create_shipping_method', () => {
  const tool = findTool(shippingZoneTools, 'create_shipping_method');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { zoneId: 'zone_001', name: 'Standard', baseRate: 9.99 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('creates method with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: {
        zoneId: 'zone_001',
        name: 'Standard',
        carrier: 'USPS',
        baseRate: 9.99,
        minDeliveryDays: 3,
        maxDeliveryDays: 7,
        currency: 'USD',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.method);
    assert.equal(result.method.name, 'Standard');
  });

  it('converts baseRate to string', async () => {
    let calledData = null;
    const commerce = makeShippingZoneCommerce({
      createMethod: async (_zoneId, data) => {
        calledData = data;
        return { ...mockMethod, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { zoneId: 'zone_001', name: 'Express', baseRate: 19.99 },
      allowApply: true,
    });
    assert.equal(typeof calledData.baseRate, 'string');
    assert.equal(calledData.baseRate, '19.99');
  });

  it('returns error when createMethod throws', async () => {
    const commerce = makeShippingZoneCommerce({
      createMethod: async () => {
        throw new Error('Zone not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { zoneId: 'bad_zone', name: 'Standard', baseRate: 9.99 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Zone not found'));
    }
  });
});

// ============================================================================
// calculate_shipping_rate
// ============================================================================

describe('calculate_shipping_rate', () => {
  const tool = findTool(shippingZoneTools, 'calculate_shipping_rate');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns rates for destination and items', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: {
        country: 'US',
        region: 'CA',
        postalCode: '90210',
        items: [{ sku: 'SKU-001', quantity: 2, weight: 500 }],
        currency: 'USD',
      },
    });
    assert.equal(result.success, true);
    assert.equal(result.destination.country, 'US');
    assert.equal(result.destination.region, 'CA');
    assert.equal(result.destination.postalCode, '90210');
    assert.equal(result.rates.length, 1);
    assert.equal(result.rates[0].methodName, 'Standard');
    assert.equal(result.rates[0].carrier, 'USPS');
    assert.equal(result.rates[0].isFreeShipping, false);
  });

  it('maps all expected fields on each rate', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: {
        country: 'US',
        items: [{ sku: 'SKU-001', quantity: 1 }],
      },
    });
    const r = result.rates[0];
    assert.ok('methodId' in r);
    assert.ok('methodName' in r);
    assert.ok('carrier' in r);
    assert.ok('rate' in r);
    assert.ok('currency' in r);
    assert.ok('minDeliveryDays' in r);
    assert.ok('maxDeliveryDays' in r);
    assert.ok('isFreeShipping' in r);
  });

  it('passes destination params to calculateRates()', async () => {
    let calledParams = null;
    const commerce = makeShippingZoneCommerce({
      calculateRates: async (params) => {
        calledParams = params;
        return [mockRate];
      },
    });
    await tool.handler({
      commerce,
      params: {
        country: 'GB',
        region: 'ENG',
        postalCode: 'SW1A 1AA',
        items: [{ sku: 'SKU-001', quantity: 1 }],
        currency: 'GBP',
      },
    });
    assert.equal(calledParams.country, 'GB');
    assert.equal(calledParams.region, 'ENG');
    assert.equal(calledParams.postalCode, 'SW1A 1AA');
    assert.equal(calledParams.currency, 'GBP');
  });

  it('returns error when calculateRates throws', async () => {
    const commerce = makeShippingZoneCommerce({
      calculateRates: async () => {
        throw new Error('No zones cover this destination');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { country: 'ZZ', items: [{ sku: 'SKU-001', quantity: 1 }] },
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('No zones cover this destination'));
    }
  });
});

// ============================================================================
// list_shipping_methods
// ============================================================================

describe('list_shipping_methods', () => {
  const tool = findTool(shippingZoneTools, 'list_shipping_methods');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns methods for a zone', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { zoneId: 'zone_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.zoneId, 'zone_001');
    assert.equal(result.count, 1);
    assert.equal(result.methods.length, 1);
    assert.equal(result.methods[0].id, 'method_001');
    assert.equal(result.methods[0].name, 'Standard');
  });

  it('maps all expected fields on each method', async () => {
    const result = await tool.handler({
      commerce: makeShippingZoneCommerce(),
      params: { zoneId: 'zone_001' },
    });
    const m = result.methods[0];
    assert.ok('id' in m);
    assert.ok('name' in m);
    assert.ok('carrier' in m);
    assert.ok('baseRate' in m);
    assert.ok('perItemRate' in m);
    assert.ok('freeShippingThreshold' in m);
    assert.ok('minDeliveryDays' in m);
    assert.ok('maxDeliveryDays' in m);
    assert.ok('currency' in m);
    assert.ok('status' in m);
  });

  it('passes zoneId to listMethods()', async () => {
    let calledZoneId = null;
    const commerce = makeShippingZoneCommerce({
      listMethods: async (zoneId) => {
        calledZoneId = zoneId;
        return [mockMethod];
      },
    });
    await tool.handler({ commerce, params: { zoneId: 'zone_999' } });
    assert.equal(calledZoneId, 'zone_999');
  });

  it('returns empty methods array when zone has none', async () => {
    const commerce = makeShippingZoneCommerce({
      listMethods: async () => [],
    });
    const result = await tool.handler({
      commerce,
      params: { zoneId: 'zone_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 0);
    assert.equal(result.methods.length, 0);
  });

  it('returns error when listMethods throws', async () => {
    const commerce = makeShippingZoneCommerce({
      listMethods: async () => {
        throw new Error('Zone not found');
      },
    });
    try {
      await tool.handler({ commerce, params: { zoneId: 'bad_zone' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Zone not found'));
    }
  });
});
