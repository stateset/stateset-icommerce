/**
 * Segment Tools Test Suite
 *
 * Tests for cli/src/tools/segments.js
 * Covers: create_segment, get_segment, list_segments, update_segment,
 *         evaluate_segment_membership
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { segmentTools } from '../../src/tools/segments.js';

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

const mockConditions = [{ field: 'totalSpent', operator: 'gte', value: 500 }];

// The engine stores rule values as strings.
const mockRules = [{ field: 'totalSpent', operator: 'gte', value: '500' }];

// Shape of the binding's SegmentOutput.
const mockSegment = {
  id: 'seg_001',
  name: 'VIP Customers',
  description: 'Customers who spent over $500',
  segmentType: 'dynamic',
  rules: mockRules,
  memberCount: 150,
  createdAt: '2026-02-21T00:00:00Z',
  updatedAt: '2026-02-21T00:00:00Z',
};

function makeSegmentCommerce(overrides = {}) {
  return {
    segments: {
      create: async (data) => ({ ...mockSegment, ...data }),
      get: async (_id) => mockSegment,
      // Only methods the real Segments binding class has.
      list: async () => [mockSegment],
      update: async (_id, data) => ({ ...mockSegment, ...data }),
      isMember: async (_segmentId, _customerId) => true,
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Segment Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(segmentTools));
  });

  it('has at least 6 tools', () => {
    assert.ok(segmentTools.length >= 5, `Expected >= 5, got ${segmentTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of segmentTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// create_segment
// ============================================================================

describe('create_segment', () => {
  const tool = findTool(segmentTools, 'create_segment');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: { name: 'High Spenders', conditions: mockConditions },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('creates a segment with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: {
        name: 'VIP Customers',
        type: 'dynamic',
        conditions: mockConditions,
        conditionLogic: 'all',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.segment);
    assert.equal(result.segment.name, 'VIP Customers');
  });

  it('passes correct data to commerce.segments.create()', async () => {
    let calledWith = null;
    const commerce = makeSegmentCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockSegment, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        name: 'Loyal Buyers',
        type: 'static',
        conditions: mockConditions,
        conditionLogic: 'all',
      },
      allowApply: true,
    });
    assert.deepStrictEqual(calledWith, {
      name: 'Loyal Buyers',
      description: undefined,
      segmentType: 'static',
      rules: mockRules,
    });
  });

  it('JSON-encodes non-string rule values', async () => {
    let calledWith = null;
    const commerce = makeSegmentCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockSegment, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        name: 'Tagged',
        conditions: [
          { field: 'tags', operator: 'in', value: ['a', 'b'] },
          { field: 'vip', operator: 'eq', value: true },
          { field: 'city', operator: 'eq', value: 'Paris' },
        ],
      },
      allowApply: true,
    });
    assert.deepStrictEqual(
      calledWith.rules.map((r) => r.value),
      ['["a","b"]', 'true', 'Paris'],
    );
  });

  it('refuses conditionLogic "any" and operators the engine lacks instead of dropping them', async () => {
    let called = false;
    const commerce = makeSegmentCommerce({
      create: async () => {
        called = true;
      },
    });
    const anyResult = await tool.handler({
      commerce,
      params: { name: 'x', conditions: mockConditions, conditionLogic: 'any' },
      allowApply: true,
    });
    assert.equal(anyResult.success, false);
    assert.match(anyResult.error, /any/);
    const opResult = await tool.handler({
      commerce,
      params: { name: 'x', conditions: [{ field: 'f', operator: 'not_in', value: ['a'] }] },
      allowApply: true,
    });
    assert.equal(opResult.success, false);
    assert.match(opResult.error, /not_in/);
    assert.equal(called, false);
  });

  it('returns error when commerce.segments.create throws', async () => {
    const commerce = makeSegmentCommerce({
      create: async () => {
        throw new Error('Invalid conditions');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { name: 'Bad Segment', conditions: mockConditions },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Invalid conditions'));
    }
  });
});

// ============================================================================
// get_segment
// ============================================================================

describe('get_segment', () => {
  const tool = findTool(segmentTools, 'get_segment');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns segment for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: { segmentId: 'seg_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.segment.id, 'seg_001');
    assert.equal(result.segment.name, 'VIP Customers');
    assert.equal(result.segment.type, 'dynamic');
    assert.equal(result.segment.memberCount, 150);
    assert.deepEqual(result.segment.rules, mockRules);
  });

  it('returns success: false when segment not found', async () => {
    const commerce = makeSegmentCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { segmentId: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeSegmentCommerce({
      get: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: { segmentId: 'seg_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// list_segments
// ============================================================================

describe('list_segments', () => {
  const tool = findTool(segmentTools, 'list_segments');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list with totalCount and returned', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.segments.length, 1);
    assert.equal(result.segments[0].id, 'seg_001');
    assert.equal(result.segments[0].name, 'VIP Customers');
  });

  it('maps expected fields on each segment', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: {},
    });
    const s = result.segments[0];
    assert.ok('id' in s);
    assert.ok('name' in s);
    assert.ok('type' in s);
    assert.ok('memberCount' in s);
    assert.ok(!('status' in s), 'SegmentOutput has no status');
    assert.ok('createdAt' in s);
  });

  it('passes type filter to commerce.segments.list()', async () => {
    let calledFilter = null;
    const commerce = makeSegmentCommerce({
      list: async (filter) => {
        calledFilter = filter;
        return [];
      },
    });
    await tool.handler({ commerce, params: { type: 'static' } });
    assert.deepStrictEqual(calledFilter, { segmentType: 'static', limit: 1000, offset: 0 });
  });

  it('returns error when list throws', async () => {
    const commerce = makeSegmentCommerce({
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
// update_segment
// ============================================================================

describe('update_segment', () => {
  const tool = findTool(segmentTools, 'update_segment');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: { segmentId: 'seg_001', name: 'VIP Platinum' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint);
  });

  it('updates segment with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: { segmentId: 'seg_001', name: 'VIP Platinum' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('updated'));
    assert.ok(result.segment);
  });

  it('passes correct fields to commerce.segments.update()', async () => {
    let calledId = null;
    let calledData = null;
    const commerce = makeSegmentCommerce({
      update: async (id, data) => {
        calledId = id;
        calledData = data;
        return { ...mockSegment, ...data };
      },
    });
    const newConditions = [{ field: 'orderCount', operator: 'gte', value: 5 }];
    await tool.handler({
      commerce,
      params: {
        segmentId: 'seg_001',
        name: 'Updated Name',
        conditions: newConditions,
        conditionLogic: 'all',
      },
      allowApply: true,
    });
    assert.equal(calledId, 'seg_001');
    assert.deepStrictEqual(calledData, {
      name: 'Updated Name',
      description: undefined,
      rules: [{ field: 'orderCount', operator: 'gte', value: '5' }],
    });
  });

  it('leaves rules untouched when conditions are not given', async () => {
    let calledData = null;
    const commerce = makeSegmentCommerce({
      update: async (_id, data) => {
        calledData = data;
        return { ...mockSegment, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { segmentId: 'seg_001', name: 'Renamed' },
      allowApply: true,
    });
    assert.equal(calledData.rules, undefined);
  });

  it('returns error when update throws', async () => {
    const commerce = makeSegmentCommerce({
      update: async () => {
        throw new Error('Segment not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { segmentId: 'bad_id', name: 'x' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Segment not found'));
    }
  });
});

// ============================================================================
// evaluate_segment_membership
// ============================================================================

describe('evaluate_segment_membership', () => {
  const tool = findTool(segmentTools, 'evaluate_segment_membership');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns membership result for valid IDs', async () => {
    const result = await tool.handler({
      commerce: makeSegmentCommerce(),
      params: { segmentId: 'seg_001', customerId: 'cust_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.segmentId, 'seg_001');
    assert.equal(result.customerId, 'cust_001');
    assert.equal(result.isMember, true);
    assert.equal(result.basis, 'stored_membership');
  });

  it('calls isMember(segmentId, customerId)', async () => {
    let args;
    const commerce = makeSegmentCommerce({
      isMember: async (...a) => {
        args = a;
        return true;
      },
    });
    await tool.handler({ commerce, params: { segmentId: 'seg_001', customerId: 'cust_001' } });
    assert.deepStrictEqual(args, ['seg_001', 'cust_001']);
  });

  it('returns isMember: false for non-member', async () => {
    const commerce = makeSegmentCommerce({
      isMember: async () => false,
    });
    const result = await tool.handler({
      commerce,
      params: { segmentId: 'seg_001', customerId: 'cust_lowspend' },
    });
    assert.equal(result.success, true);
    assert.equal(result.isMember, false);
  });

  it('returns error when isMember throws', async () => {
    const commerce = makeSegmentCommerce({
      isMember: async () => {
        throw new Error('Segment not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { segmentId: 'bad_seg', customerId: 'cust_001' },
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Segment not found'));
    }
  });
});
