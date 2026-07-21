/**
 * Finance Operations Tools Test Suite
 *
 * Tests for the fixed-assets, revenue-recognition, and cycle-counts tool
 * modules, plus the three_way_match_bill and revalue_gl extensions.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { fixedAssetTools } from '../../src/tools/fixed-assets.js';
import { revenueRecognitionTools } from '../../src/tools/revenue-recognition.js';
import { cycleCountTools } from '../../src/tools/cycle-counts.js';
import { accountsPayableTools } from '../../src/tools/accounts-payable.js';
import { generalLedgerTools } from '../../src/tools/general-ledger.js';
import { DOMAIN_TOOL_ARRAYS, TOOL_POLICY_DOMAIN_BY_NAME } from '../../src/tools/domain-registry.js';

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// REGISTRY
// ============================================================================

describe('Finance ops domain registry', () => {
  it('registers the new modules', () => {
    assert.equal(DOMAIN_TOOL_ARRAYS['fixed-assets'], fixedAssetTools);
    assert.equal(DOMAIN_TOOL_ARRAYS['revenue-recognition'], revenueRecognitionTools);
    assert.equal(DOMAIN_TOOL_ARRAYS['cycle-counts'], cycleCountTools);
  });

  it('assigns policy domains', () => {
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.list_fixed_assets, 'fixed_assets');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.recognize_revenue, 'revenue_recognition');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.record_cycle_counts, 'cycle_counts');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.three_way_match_bill, 'accounts_payable');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.revalue_gl, 'general_ledger');
  });

  it('exposes the expected tool names', () => {
    assert.deepEqual(
      fixedAssetTools.map((t) => t.name),
      [
        'list_fixed_assets',
        'get_fixed_asset',
        'create_fixed_asset',
        'place_asset_in_service',
        'dispose_fixed_asset',
        'write_off_fixed_asset',
        'generate_depreciation_schedule',
        'get_depreciation_schedule',
        'post_depreciation',
      ],
    );
    assert.deepEqual(
      revenueRecognitionTools.map((t) => t.name),
      [
        'list_revenue_contracts',
        'get_revenue_contract',
        'create_revenue_contract',
        'generate_revenue_schedule',
        'get_revenue_schedule',
        'recognize_revenue',
      ],
    );
    assert.deepEqual(
      cycleCountTools.map((t) => t.name),
      [
        'list_cycle_counts',
        'get_cycle_count',
        'create_cycle_count',
        'start_cycle_count',
        'record_cycle_counts',
        'complete_cycle_count',
        'cancel_cycle_count',
      ],
    );
  });

  it('marks write tools as write permission and read tools as read', () => {
    const readNames = new Set([
      'list_fixed_assets',
      'get_fixed_asset',
      'get_depreciation_schedule',
      'list_revenue_contracts',
      'get_revenue_contract',
      'get_revenue_schedule',
      'list_cycle_counts',
      'get_cycle_count',
    ]);
    for (const tool of [...fixedAssetTools, ...revenueRecognitionTools, ...cycleCountTools]) {
      assert.equal(tool.permission, readNames.has(tool.name) ? 'read' : 'write', tool.name);
    }
    assert.equal(findTool(accountsPayableTools, 'three_way_match_bill').permission, 'read');
    assert.equal(findTool(generalLedgerTools, 'revalue_gl').permission, 'write');
  });
});

// ============================================================================
// APPLY GUARD
// ============================================================================

describe('Finance ops apply guard', () => {
  it('write tools refuse to mutate without allowApply', async () => {
    const writeTools = [
      ...fixedAssetTools,
      ...revenueRecognitionTools,
      ...cycleCountTools,
      findTool(generalLedgerTools, 'revalue_gl'),
    ].filter((t) => t.permission === 'write');

    for (const tool of writeTools) {
      let called = false;
      const trap = new Proxy(
        {},
        {
          get: () => async () => {
            called = true;
            return {};
          },
        },
      );
      const commerce = {
        fixedAssets: trap,
        revenueRecognition: trap,
        cycleCounts: trap,
        generalLedger: trap,
      };
      const result = await tool.handler({ commerce, params: {}, allowApply: false });
      assert.equal(result.success, false, tool.name);
      assert.equal(called, false, `${tool.name} must not call the API without allowApply`);
    }
  });
});

// ============================================================================
// FIXED ASSETS
// ============================================================================

describe('Fixed Asset Tools', () => {
  const mockAsset = {
    id: 'fa_001',
    name: 'Forklift',
    assetType: 'equipment',
    acquisitionCost: '25000.00',
    acquisitionDate: '2026-01-15T00:00:00Z',
    status: 'draft',
  };
  const mockSchedule = {
    assetId: 'fa_001',
    periods: [{ periodDate: '2026-02-01', amount: '416.67' }],
  };

  function makeCommerce(overrides = {}) {
    return {
      fixedAssets: {
        list: async () => [mockAsset],
        get: async () => mockAsset,
        create: async (data) => ({ ...mockAsset, ...data }),
        placeInService: async (id, date) => ({
          ...mockAsset,
          id,
          status: 'in_service',
          inServiceDate: date,
        }),
        dispose: async (id, data) => ({ ...mockAsset, id, status: 'disposed', ...data }),
        writeOff: async (id, reason) => ({ ...mockAsset, id, status: 'written_off', reason }),
        generateSchedule: async () => mockSchedule,
        getSchedule: async () => mockSchedule,
        postDepreciation: async (id, periodDate) => ({ assetId: id, periodDate, amount: '416.67' }),
        ...overrides,
      },
    };
  }

  it('list_fixed_assets returns assets with count', async () => {
    const result = await findTool(fixedAssetTools, 'list_fixed_assets').handler({
      commerce: makeCommerce(),
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.assets[0].id, 'fa_001');
  });

  it('get_fixed_asset returns error when not found', async () => {
    const result = await findTool(fixedAssetTools, 'get_fixed_asset').handler({
      commerce: makeCommerce({ get: async () => null }),
      params: { assetId: 'missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Fixed asset not found');
  });

  it('create_fixed_asset passes decimal strings through', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockAsset;
      },
    });
    const result = await findTool(fixedAssetTools, 'create_fixed_asset').handler({
      commerce,
      params: {
        name: 'Forklift',
        assetType: 'equipment',
        acquisitionCost: '25000.00',
        acquisitionDate: '2026-01-15T00:00:00Z',
        salvageValue: '1000.00',
        usefulLifeMonths: 60,
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.acquisitionCost, '25000.00');
    assert.equal(received.salvageValue, '1000.00');
    assert.equal(received.usefulLifeMonths, 60);
  });

  it('place_asset_in_service forwards id and date', async () => {
    const result = await findTool(fixedAssetTools, 'place_asset_in_service').handler({
      commerce: makeCommerce(),
      params: { assetId: 'fa_001', inServiceDate: '2026-02-01T00:00:00Z' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.asset.status, 'in_service');
    assert.equal(result.asset.inServiceDate, '2026-02-01T00:00:00Z');
  });

  it('dispose_fixed_asset forwards disposal details', async () => {
    let received;
    const commerce = makeCommerce({
      dispose: async (id, data) => {
        received = { id, ...data };
        return { ...mockAsset, status: 'disposed' };
      },
    });
    const result = await findTool(fixedAssetTools, 'dispose_fixed_asset').handler({
      commerce,
      params: {
        assetId: 'fa_001',
        disposalDate: '2026-06-01T00:00:00Z',
        disposalProceeds: '5000.00',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.id, 'fa_001');
    assert.equal(received.disposalProceeds, '5000.00');
  });

  it('write_off_fixed_asset forwards reason', async () => {
    const result = await findTool(fixedAssetTools, 'write_off_fixed_asset').handler({
      commerce: makeCommerce(),
      params: { assetId: 'fa_001', reason: 'Damaged beyond repair' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.asset.status, 'written_off');
  });

  it('generate_depreciation_schedule returns schedule', async () => {
    const result = await findTool(fixedAssetTools, 'generate_depreciation_schedule').handler({
      commerce: makeCommerce(),
      params: { assetId: 'fa_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.schedule.periods.length, 1);
  });

  it('get_depreciation_schedule returns error when missing', async () => {
    const result = await findTool(fixedAssetTools, 'get_depreciation_schedule').handler({
      commerce: makeCommerce({ getSchedule: async () => null }),
      params: { assetId: 'fa_001' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Depreciation schedule not found');
  });

  it('post_depreciation forwards period date', async () => {
    const result = await findTool(fixedAssetTools, 'post_depreciation').handler({
      commerce: makeCommerce(),
      params: { assetId: 'fa_001', periodDate: '2026-02-01T00:00:00Z' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.result.periodDate, '2026-02-01T00:00:00Z');
  });
});

// ============================================================================
// REVENUE RECOGNITION
// ============================================================================

describe('Revenue Recognition Tools', () => {
  const mockContract = {
    id: 'rc_001',
    customerId: 'cust_001',
    totalValue: '12000.00',
    startDate: '2026-01-01T00:00:00Z',
    status: 'active',
  };
  const mockSchedule = {
    contractId: 'rc_001',
    periods: [{ periodDate: '2026-01-31', amount: '1000.00' }],
  };

  function makeCommerce(overrides = {}) {
    return {
      revenueRecognition: {
        listContracts: async () => [mockContract],
        getContract: async () => mockContract,
        createContract: async (data) => ({ ...mockContract, ...data }),
        generateSchedule: async () => mockSchedule,
        getSchedule: async () => mockSchedule,
        recognize: async (id, periodDate) => ({
          contractId: id,
          periodDate,
          recognized: '1000.00',
        }),
        ...overrides,
      },
    };
  }

  it('list_revenue_contracts returns contracts with count', async () => {
    const result = await findTool(revenueRecognitionTools, 'list_revenue_contracts').handler({
      commerce: makeCommerce(),
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
  });

  it('get_revenue_contract returns error when not found', async () => {
    const result = await findTool(revenueRecognitionTools, 'get_revenue_contract').handler({
      commerce: makeCommerce({ getContract: async () => null }),
      params: { contractId: 'missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Revenue contract not found');
  });

  it('create_revenue_contract passes decimal total value', async () => {
    let received;
    const commerce = makeCommerce({
      createContract: async (data) => {
        received = data;
        return mockContract;
      },
    });
    const result = await findTool(revenueRecognitionTools, 'create_revenue_contract').handler({
      commerce,
      params: {
        customerId: 'cust_001',
        totalValue: '12000.00',
        startDate: '2026-01-01T00:00:00Z',
        endDate: '2026-12-31T00:00:00Z',
        currency: 'USD',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.totalValue, '12000.00');
    assert.equal(received.currency, 'USD');
  });

  it('generate_revenue_schedule returns schedule', async () => {
    const result = await findTool(revenueRecognitionTools, 'generate_revenue_schedule').handler({
      commerce: makeCommerce(),
      params: { contractId: 'rc_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.schedule.contractId, 'rc_001');
  });

  it('get_revenue_schedule returns error when missing', async () => {
    const result = await findTool(revenueRecognitionTools, 'get_revenue_schedule').handler({
      commerce: makeCommerce({ getSchedule: async () => null }),
      params: { contractId: 'rc_001' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Revenue schedule not found');
  });

  it('recognize_revenue forwards contract and period', async () => {
    const result = await findTool(revenueRecognitionTools, 'recognize_revenue').handler({
      commerce: makeCommerce(),
      params: { contractId: 'rc_001', periodDate: '2026-01-31T00:00:00Z' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.result.recognized, '1000.00');
  });
});

// ============================================================================
// CYCLE COUNTS
// ============================================================================

describe('Cycle Count Tools', () => {
  const mockCycleCount = {
    id: 'cc_001',
    warehouseId: 1,
    status: 'scheduled',
    lines: [{ sku: 'WIDGET-001', expectedQuantity: 10 }],
  };

  function makeCommerce(overrides = {}) {
    return {
      cycleCounts: {
        list: async () => [mockCycleCount],
        get: async () => mockCycleCount,
        create: async (data) => ({ ...mockCycleCount, ...data }),
        start: async (id) => ({ ...mockCycleCount, id, status: 'in_progress' }),
        recordCounts: async (id, counts) => ({ ...mockCycleCount, id, counts }),
        complete: async (id) => ({ ...mockCycleCount, id, status: 'completed' }),
        cancel: async (id) => ({ ...mockCycleCount, id, status: 'canceled' }),
        ...overrides,
      },
    };
  }

  it('list_cycle_counts returns cycle counts with count', async () => {
    const result = await findTool(cycleCountTools, 'list_cycle_counts').handler({
      commerce: makeCommerce(),
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
  });

  it('get_cycle_count returns error when not found', async () => {
    const result = await findTool(cycleCountTools, 'get_cycle_count').handler({
      commerce: makeCommerce({ get: async () => null }),
      params: { cycleCountId: 'missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Cycle count not found');
  });

  it('create_cycle_count forwards fields', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockCycleCount;
      },
    });
    const result = await findTool(cycleCountTools, 'create_cycle_count').handler({
      commerce,
      params: { warehouseId: 1, skus: ['WIDGET-001'], assignedTo: 'alice' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(received.skus, ['WIDGET-001']);
    assert.equal(received.assignedTo, 'alice');
  });

  it('start_cycle_count transitions status', async () => {
    const result = await findTool(cycleCountTools, 'start_cycle_count').handler({
      commerce: makeCommerce(),
      params: { cycleCountId: 'cc_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.cycleCount.status, 'in_progress');
  });

  it('record_cycle_counts forwards counted lines', async () => {
    let received;
    const commerce = makeCommerce({
      recordCounts: async (id, counts) => {
        received = { id, counts };
        return mockCycleCount;
      },
    });
    const counts = [{ sku: 'WIDGET-001', countedQuantity: 9, countedBy: 'alice' }];
    const result = await findTool(cycleCountTools, 'record_cycle_counts').handler({
      commerce,
      params: { cycleCountId: 'cc_001', counts },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.id, 'cc_001');
    assert.deepEqual(received.counts, counts);
  });

  it('complete_cycle_count transitions status', async () => {
    const result = await findTool(cycleCountTools, 'complete_cycle_count').handler({
      commerce: makeCommerce(),
      params: { cycleCountId: 'cc_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.cycleCount.status, 'completed');
  });

  it('cancel_cycle_count transitions status', async () => {
    const result = await findTool(cycleCountTools, 'cancel_cycle_count').handler({
      commerce: makeCommerce(),
      params: { cycleCountId: 'cc_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.cycleCount.status, 'canceled');
  });
});

// ============================================================================
// AP THREE-WAY MATCH + GL REVALUATION
// ============================================================================

describe('three_way_match_bill', () => {
  const tool = findTool(accountsPayableTools, 'three_way_match_bill');

  it('runs the match without allowApply (compute-on-read)', async () => {
    let received;
    const commerce = {
      accountsPayable: {
        threeWayMatch: async (billId, tolerancePercent) => {
          received = { billId, tolerancePercent };
          return { matched: true, variances: [] };
        },
      },
    };
    const result = await tool.handler({
      commerce,
      params: { billId: 'bill_001', tolerancePercent: 2 },
    });
    assert.equal(result.success, true);
    assert.equal(result.match.matched, true);
    assert.deepEqual(received, { billId: 'bill_001', tolerancePercent: 2 });
  });

  it('omits tolerance when not provided', async () => {
    let received;
    const commerce = {
      accountsPayable: {
        threeWayMatch: async (billId, tolerancePercent) => {
          received = { billId, tolerancePercent };
          return { matched: false, variances: [{ field: 'quantity' }] };
        },
      },
    };
    const result = await tool.handler({ commerce, params: { billId: 'bill_002' } });
    assert.equal(result.success, true);
    assert.equal(received.tolerancePercent, undefined);
    assert.equal(result.match.matched, false);
  });
});

describe('revalue_gl', () => {
  const tool = findTool(generalLedgerTools, 'revalue_gl');

  it('requires allowApply', async () => {
    let called = false;
    const commerce = {
      generalLedger: {
        revalue: async () => {
          called = true;
          return {};
        },
      },
    };
    const result = await tool.handler({
      commerce,
      params: { asOfDate: '2026-06-30T00:00:00Z' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.equal(called, false);
  });

  it('revalues with asOfDate and baseCurrency', async () => {
    let received;
    const commerce = {
      generalLedger: {
        revalue: async (asOfDate, baseCurrency) => {
          received = { asOfDate, baseCurrency };
          return { adjustments: [{ accountId: 'gl_001', amount: '-12.34' }] };
        },
      },
    };
    const result = await tool.handler({
      commerce,
      params: { asOfDate: '2026-06-30T00:00:00Z', baseCurrency: 'USD' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(received, { asOfDate: '2026-06-30T00:00:00Z', baseCurrency: 'USD' });
    assert.equal(result.revaluation.adjustments.length, 1);
  });
});
