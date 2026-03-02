import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { shippingZoneTools } from '../../src/tools/shipping-zones.js';

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(shippingZoneTools.map((t) => [t.name, t]));

const EXPECTED_NAMES = [
  'create_shipping_zone',
  'get_shipping_zone',
  'list_shipping_zones',
  'update_shipping_zone',
  'create_shipping_method',
  'calculate_shipping_rate',
  'list_shipping_methods',
];

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('shippingZoneTools — module exports', () => {
  it('exports an array of 7 tools', () => {
    assert.ok(Array.isArray(shippingZoneTools));
    assert.equal(shippingZoneTools.length, 7);
  });

  it('exports expected tool names in order', () => {
    const names = shippingZoneTools.map((t) => t.name);
    assert.deepStrictEqual(names, EXPECTED_NAMES);
  });

  it('all tools have handler functions', () => {
    for (const tool of shippingZoneTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of shippingZoneTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of shippingZoneTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have an inputSchema object', () => {
    for (const tool of shippingZoneTools) {
      assert.ok(
        tool.inputSchema && typeof tool.inputSchema === 'object',
        `${tool.name} missing inputSchema`,
      );
    }
  });
});

// ---------------------------------------------------------------------------
// Permission checks
// ---------------------------------------------------------------------------

describe('shippingZoneTools — permission assignments', () => {
  it('create_shipping_zone is write', () => {
    assert.equal(byName['create_shipping_zone'].permission, 'write');
  });

  it('get_shipping_zone is read', () => {
    assert.equal(byName['get_shipping_zone'].permission, 'read');
  });

  it('list_shipping_zones is read', () => {
    assert.equal(byName['list_shipping_zones'].permission, 'read');
  });

  it('update_shipping_zone is write', () => {
    assert.equal(byName['update_shipping_zone'].permission, 'write');
  });

  it('create_shipping_method is write', () => {
    assert.equal(byName['create_shipping_method'].permission, 'write');
  });

  it('calculate_shipping_rate is read', () => {
    assert.equal(byName['calculate_shipping_rate'].permission, 'read');
  });

  it('list_shipping_methods is read', () => {
    assert.equal(byName['list_shipping_methods'].permission, 'read');
  });
});

// ---------------------------------------------------------------------------
// Input schema validation
// ---------------------------------------------------------------------------

describe('shippingZoneTools — input schemas', () => {
  it('create_shipping_zone has name, countries, regions, postalCodeRanges', () => {
    const schema = byName['create_shipping_zone'].inputSchema;
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.countries, 'missing countries');
    assert.ok(schema.regions, 'missing regions');
    assert.ok(schema.postalCodeRanges, 'missing postalCodeRanges');
  });

  it('get_shipping_zone has zoneId', () => {
    const schema = byName['get_shipping_zone'].inputSchema;
    assert.ok(schema.zoneId, 'missing zoneId');
  });

  it('list_shipping_zones has optional limit', () => {
    const schema = byName['list_shipping_zones'].inputSchema;
    assert.ok(schema.limit, 'missing limit');
  });

  it('update_shipping_zone has zoneId, and optional name, countries, regions, postalCodeRanges', () => {
    const schema = byName['update_shipping_zone'].inputSchema;
    assert.ok(schema.zoneId, 'missing zoneId');
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.countries, 'missing countries');
    assert.ok(schema.regions, 'missing regions');
    assert.ok(schema.postalCodeRanges, 'missing postalCodeRanges');
  });

  it('create_shipping_method has zoneId, name, carrier, baseRate, and optional fields', () => {
    const schema = byName['create_shipping_method'].inputSchema;
    assert.ok(schema.zoneId, 'missing zoneId');
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.carrier, 'missing carrier');
    assert.ok(schema.minDeliveryDays, 'missing minDeliveryDays');
    assert.ok(schema.maxDeliveryDays, 'missing maxDeliveryDays');
    assert.ok(schema.baseRate, 'missing baseRate');
    assert.ok(schema.perItemRate, 'missing perItemRate');
    assert.ok(schema.freeShippingThreshold, 'missing freeShippingThreshold');
    assert.ok(schema.currency, 'missing currency');
  });

  it('calculate_shipping_rate has country, region, postalCode, items, currency', () => {
    const schema = byName['calculate_shipping_rate'].inputSchema;
    assert.ok(schema.country, 'missing country');
    assert.ok(schema.region, 'missing region');
    assert.ok(schema.postalCode, 'missing postalCode');
    assert.ok(schema.items, 'missing items');
    assert.ok(schema.currency, 'missing currency');
  });

  it('list_shipping_methods has zoneId', () => {
    const schema = byName['list_shipping_methods'].inputSchema;
    assert.ok(schema.zoneId, 'missing zoneId');
  });
});

// ---------------------------------------------------------------------------
// Handler apply-guard (write tools)
// ---------------------------------------------------------------------------

describe('shippingZoneTools — apply-guard on write tools', () => {
  it('create_shipping_zone requires --apply', async () => {
    const result = await byName['create_shipping_zone'].handler({
      params: { name: 'Domestic', countries: ['US'] },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
  });

  it('update_shipping_zone requires --apply', async () => {
    const result = await byName['update_shipping_zone'].handler({
      params: { zoneId: 'zone-1', name: 'Updated Zone' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('create_shipping_method requires --apply', async () => {
    const result = await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-1', name: 'Standard', baseRate: 5.99 },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });
});

// ---------------------------------------------------------------------------
// Handler error paths (commerce stub missing methods)
// ---------------------------------------------------------------------------

describe('shippingZoneTools — handler error paths', () => {
  it('get_shipping_zone fails gracefully with empty commerce', async () => {
    try {
      await byName['get_shipping_zone'].handler({
        params: { zoneId: 'zone-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('list_shipping_zones fails gracefully with empty commerce', async () => {
    try {
      await byName['list_shipping_zones'].handler({
        params: { limit: 50 },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('calculate_shipping_rate fails gracefully with empty commerce', async () => {
    try {
      await byName['calculate_shipping_rate'].handler({
        params: {
          country: 'US',
          items: [{ sku: 'WIDGET-001', quantity: 1 }],
        },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('list_shipping_methods fails gracefully with empty commerce', async () => {
    try {
      await byName['list_shipping_methods'].handler({
        params: { zoneId: 'zone-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('create_shipping_zone fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['create_shipping_zone'].handler({
        params: { name: 'Test Zone', countries: ['US'] },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('update_shipping_zone fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['update_shipping_zone'].handler({
        params: { zoneId: 'zone-1', name: 'Updated' },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('create_shipping_method fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['create_shipping_method'].handler({
        params: { zoneId: 'zone-1', name: 'Express', baseRate: 12.99 },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });
});

// ---------------------------------------------------------------------------
// Handler success paths (mocked commerce)
// ---------------------------------------------------------------------------

describe('shippingZoneTools — handler success paths (mocked commerce)', () => {
  const mockZone = {
    id: 'zone-001',
    name: 'Domestic US',
    countries: ['US'],
    regions: ['CA', 'NY'],
    postalCodeRanges: [],
    methods: [],
    methodCount: 2,
    status: 'active',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-15T00:00:00Z',
  };

  const mockMethod = {
    id: 'meth-001',
    name: 'Standard',
    carrier: 'USPS',
    baseRate: '5.99',
    perItemRate: '0',
    freeShippingThreshold: '50.00',
    minDeliveryDays: 3,
    maxDeliveryDays: 7,
    currency: 'USD',
    status: 'active',
  };

  const mockRates = [
    {
      methodId: 'meth-001',
      methodName: 'Standard',
      carrier: 'USPS',
      rate: '5.99',
      currency: 'USD',
      minDeliveryDays: 3,
      maxDeliveryDays: 7,
      isFreeShipping: false,
    },
    {
      methodId: 'meth-002',
      methodName: 'Express',
      carrier: 'FedEx',
      rate: '14.99',
      currency: 'USD',
      minDeliveryDays: 1,
      maxDeliveryDays: 2,
      isFreeShipping: false,
    },
  ];

  const commerce = {
    shippingZones: {
      create: async (data) => ({ id: 'zone-new', ...data }),
      get: async (id) => (id === 'zone-001' ? mockZone : null),
      list: async () => [mockZone],
      count: async () => 1,
      update: async (id, data) => ({ ...mockZone, id, ...data }),
      createMethod: async (_zoneId, data) => ({ id: 'meth-new', ...data }),
      calculateRates: async () => mockRates,
      listMethods: async () => [mockMethod],
    },
  };

  it('create_shipping_zone returns success with allowApply', async () => {
    const result = await byName['create_shipping_zone'].handler({
      params: { name: 'EU Zone', countries: ['DE', 'FR', 'IT'] },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.zone);
  });

  it('get_shipping_zone returns success for existing zone', async () => {
    const result = await byName['get_shipping_zone'].handler({
      params: { zoneId: 'zone-001' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.zone);
    assert.equal(result.zone.id, 'zone-001');
    assert.equal(result.zone.name, 'Domestic US');
    assert.deepStrictEqual(result.zone.countries, ['US']);
    assert.ok(Array.isArray(result.zone.methods));
  });

  it('get_shipping_zone returns not-found for missing zone', async () => {
    const result = await byName['get_shipping_zone'].handler({
      params: { zoneId: 'nonexistent' },
      commerce,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('list_shipping_zones returns zones with totalCount', async () => {
    const result = await byName['list_shipping_zones'].handler({
      params: { limit: 50 },
      commerce,
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.zones));
    assert.equal(result.zones[0].name, 'Domestic US');
  });

  it('update_shipping_zone returns success with allowApply', async () => {
    const result = await byName['update_shipping_zone'].handler({
      params: { zoneId: 'zone-001', name: 'US Domestic' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('updated'));
    assert.ok(result.zone);
  });

  it('create_shipping_method returns success with allowApply', async () => {
    const result = await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-001', name: 'Express', baseRate: 12.99 },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.method);
  });

  it('calculate_shipping_rate returns rates with destination', async () => {
    const result = await byName['calculate_shipping_rate'].handler({
      params: {
        country: 'US',
        region: 'CA',
        postalCode: '90210',
        items: [{ sku: 'WIDGET-001', quantity: 2 }],
      },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.destination);
    assert.equal(result.destination.country, 'US');
    assert.equal(result.destination.region, 'CA');
    assert.equal(result.destination.postalCode, '90210');
    assert.ok(Array.isArray(result.rates));
    assert.equal(result.rates.length, 2);
    assert.equal(result.rates[0].methodName, 'Standard');
    assert.equal(result.rates[1].methodName, 'Express');
  });

  it('list_shipping_methods returns methods for zone', async () => {
    const result = await byName['list_shipping_methods'].handler({
      params: { zoneId: 'zone-001' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.equal(result.zoneId, 'zone-001');
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.methods));
    assert.equal(result.methods[0].name, 'Standard');
    assert.equal(result.methods[0].carrier, 'USPS');
  });
});

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

describe('shippingZoneTools — edge cases', () => {
  it('create_shipping_method converts baseRate to string', async () => {
    let capturedData;
    const commerce = {
      shippingZones: {
        createMethod: async (_zoneId, data) => {
          capturedData = data;
          return { id: 'meth-new', ...data };
        },
      },
    };

    await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-1', name: 'Ground', baseRate: 7.5, perItemRate: 1.25 },
      allowApply: true,
      commerce,
    });

    assert.equal(capturedData.baseRate, '7.5');
    assert.equal(capturedData.perItemRate, '1.25');
  });

  it('create_shipping_method defaults perItemRate to "0"', async () => {
    let capturedData;
    const commerce = {
      shippingZones: {
        createMethod: async (_zoneId, data) => {
          capturedData = data;
          return { id: 'meth-new', ...data };
        },
      },
    };

    await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-1', name: 'Ground', baseRate: 5.0 },
      allowApply: true,
      commerce,
    });

    assert.equal(capturedData.perItemRate, '0');
  });

  it('create_shipping_method converts freeShippingThreshold to string', async () => {
    let capturedData;
    const commerce = {
      shippingZones: {
        createMethod: async (_zoneId, data) => {
          capturedData = data;
          return { id: 'meth-new', ...data };
        },
      },
    };

    await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-1', name: 'Ground', baseRate: 5.0, freeShippingThreshold: 50 },
      allowApply: true,
      commerce,
    });

    assert.equal(capturedData.freeShippingThreshold, '50');
  });

  it('create_shipping_method leaves freeShippingThreshold undefined when not provided', async () => {
    let capturedData;
    const commerce = {
      shippingZones: {
        createMethod: async (_zoneId, data) => {
          capturedData = data;
          return { id: 'meth-new', ...data };
        },
      },
    };

    await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-1', name: 'Ground', baseRate: 5.0 },
      allowApply: true,
      commerce,
    });

    assert.equal(capturedData.freeShippingThreshold, undefined);
  });

  it('create_shipping_method defaults currency to USD', async () => {
    let capturedData;
    const commerce = {
      shippingZones: {
        createMethod: async (_zoneId, data) => {
          capturedData = data;
          return { id: 'meth-new', ...data };
        },
      },
    };

    await byName['create_shipping_method'].handler({
      params: { zoneId: 'zone-1', name: 'Ground', baseRate: 5.0 },
      allowApply: true,
      commerce,
    });

    assert.equal(capturedData.currency, 'USD');
  });

  it('calculate_shipping_rate defaults currency to USD', async () => {
    let capturedParams;
    const commerce = {
      shippingZones: {
        calculateRates: async (data) => {
          capturedParams = data;
          return [];
        },
      },
    };

    await byName['calculate_shipping_rate'].handler({
      params: {
        country: 'US',
        items: [{ sku: 'W-1', quantity: 1 }],
      },
      commerce,
    });

    assert.equal(capturedParams.currency, 'USD');
  });
});
