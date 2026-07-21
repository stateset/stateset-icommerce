import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { segmentTools } from '../../src/tools/segments.js';

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(segmentTools.map((t) => [t.name, t]));

const EXPECTED_NAMES = [
  'create_segment',
  'get_segment',
  'list_segments',
  'update_segment',
  'evaluate_segment_membership',
  'rebuild_dynamic_segment',
];

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('segmentTools — module exports', () => {
  it('exports an array of 6 tools', () => {
    assert.ok(Array.isArray(segmentTools));
    assert.equal(segmentTools.length, 6);
  });

  it('exports expected tool names in order', () => {
    const names = segmentTools.map((t) => t.name);
    assert.deepStrictEqual(names, EXPECTED_NAMES);
  });

  it('all tools have handler functions', () => {
    for (const tool of segmentTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of segmentTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of segmentTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have an inputSchema object', () => {
    for (const tool of segmentTools) {
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

describe('segmentTools — permission assignments', () => {
  it('create_segment is write', () => {
    assert.equal(byName['create_segment'].permission, 'write');
  });

  it('get_segment is read', () => {
    assert.equal(byName['get_segment'].permission, 'read');
  });

  it('list_segments is read', () => {
    assert.equal(byName['list_segments'].permission, 'read');
  });

  it('update_segment is write', () => {
    assert.equal(byName['update_segment'].permission, 'write');
  });

  it('evaluate_segment_membership is read', () => {
    assert.equal(byName['evaluate_segment_membership'].permission, 'read');
  });

  it('rebuild_dynamic_segment is write', () => {
    assert.equal(byName['rebuild_dynamic_segment'].permission, 'write');
  });
});

// ---------------------------------------------------------------------------
// Input schema validation
// ---------------------------------------------------------------------------

describe('segmentTools — input schemas', () => {
  it('create_segment has name, description, type, conditions, conditionLogic', () => {
    const schema = byName['create_segment'].inputSchema;
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.description, 'missing description');
    assert.ok(schema.type, 'missing type');
    assert.ok(schema.conditions, 'missing conditions');
    assert.ok(schema.conditionLogic, 'missing conditionLogic');
  });

  it('get_segment has segmentId', () => {
    const schema = byName['get_segment'].inputSchema;
    assert.ok(schema.segmentId, 'missing segmentId');
  });

  it('list_segments has optional type and limit', () => {
    const schema = byName['list_segments'].inputSchema;
    assert.ok(schema.type, 'missing type');
    assert.ok(schema.limit, 'missing limit');
  });

  it('update_segment has segmentId, and optional name, description, conditions, conditionLogic', () => {
    const schema = byName['update_segment'].inputSchema;
    assert.ok(schema.segmentId, 'missing segmentId');
    assert.ok(schema.name, 'missing name');
    assert.ok(schema.description, 'missing description');
    assert.ok(schema.conditions, 'missing conditions');
    assert.ok(schema.conditionLogic, 'missing conditionLogic');
  });

  it('evaluate_segment_membership has segmentId and customerId', () => {
    const schema = byName['evaluate_segment_membership'].inputSchema;
    assert.ok(schema.segmentId, 'missing segmentId');
    assert.ok(schema.customerId, 'missing customerId');
  });

  it('rebuild_dynamic_segment has segmentId', () => {
    const schema = byName['rebuild_dynamic_segment'].inputSchema;
    assert.ok(schema.segmentId, 'missing segmentId');
  });
});

// ---------------------------------------------------------------------------
// Handler apply-guard (write tools)
// ---------------------------------------------------------------------------

describe('segmentTools — apply-guard on write tools', () => {
  const sampleConditions = [{ field: 'totalSpent', operator: 'gt', value: 100 }];

  it('create_segment requires --apply', async () => {
    const result = await byName['create_segment'].handler({
      params: { name: 'VIP Customers', conditions: sampleConditions },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
  });

  it('update_segment requires --apply', async () => {
    const result = await byName['update_segment'].handler({
      params: { segmentId: 'seg-1', name: 'Updated VIP' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('rebuild_dynamic_segment requires --apply', async () => {
    const result = await byName['rebuild_dynamic_segment'].handler({
      params: { segmentId: 'seg-1' },
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

describe('segmentTools — handler error paths', () => {
  it('get_segment fails gracefully with empty commerce', async () => {
    try {
      await byName['get_segment'].handler({
        params: { segmentId: 'seg-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('list_segments fails gracefully with empty commerce', async () => {
    try {
      await byName['list_segments'].handler({
        params: { limit: 50 },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('evaluate_segment_membership fails gracefully with empty commerce', async () => {
    try {
      await byName['evaluate_segment_membership'].handler({
        params: { segmentId: 'seg-1', customerId: 'cust-1' },
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('create_segment fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['create_segment'].handler({
        params: { name: 'Test', conditions: [{ field: 'orderCount', operator: 'gt', value: 5 }] },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('update_segment fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['update_segment'].handler({
        params: { segmentId: 'seg-1', name: 'Updated' },
        allowApply: true,
        commerce: {},
      });
      assert.fail('Expected an error to be thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });

  it('rebuild_dynamic_segment fails gracefully with empty commerce when allowApply=true', async () => {
    try {
      await byName['rebuild_dynamic_segment'].handler({
        params: { segmentId: 'seg-1' },
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

describe('segmentTools — handler success paths (mocked commerce)', () => {
  const mockSegment = {
    id: 'seg-001',
    name: 'VIP Customers',
    description: 'Customers who spent over $1000',
    type: 'dynamic',
    conditions: [{ field: 'totalSpent', operator: 'gt', value: 1000 }],
    conditionLogic: 'all',
    memberCount: 128,
    status: 'active',
    lastEvaluatedAt: '2026-01-15T12:00:00Z',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-15T12:00:00Z',
  };

  const mockMembership = {
    isMember: true,
    matchedConditions: ['totalSpent > 1000'],
    evaluatedAt: '2026-01-15T12:00:00Z',
  };

  const mockRebuild = {
    memberCount: 130,
    added: 5,
    removed: 3,
    evaluatedAt: '2026-01-15T12:30:00Z',
  };

  const commerce = {
    segments: {
      create: async (data) => ({ id: 'seg-new', ...data }),
      get: async (id) => (id === 'seg-001' ? mockSegment : null),
      list: async () => [mockSegment],
      count: async () => 1,
      update: async (id, data) => ({ ...mockSegment, id, ...data }),
      evaluateMembership: async () => mockMembership,
      rebuild: async () => mockRebuild,
    },
  };

  it('create_segment returns success with allowApply', async () => {
    const result = await byName['create_segment'].handler({
      params: {
        name: 'High Spenders',
        conditions: [{ field: 'totalSpent', operator: 'gt', value: 500 }],
      },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.segment);
  });

  it('get_segment returns success for existing segment', async () => {
    const result = await byName['get_segment'].handler({
      params: { segmentId: 'seg-001' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.segment);
    assert.equal(result.segment.id, 'seg-001');
    assert.equal(result.segment.name, 'VIP Customers');
    assert.equal(result.segment.memberCount, 128);
    assert.equal(result.segment.type, 'dynamic');
  });

  it('get_segment returns not-found for missing segment', async () => {
    const result = await byName['get_segment'].handler({
      params: { segmentId: 'nonexistent' },
      commerce,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('list_segments returns segments with totalCount', async () => {
    const result = await byName['list_segments'].handler({
      params: { limit: 50 },
      commerce,
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.segments));
    assert.equal(result.segments[0].name, 'VIP Customers');
  });

  it('update_segment returns success with allowApply', async () => {
    const result = await byName['update_segment'].handler({
      params: { segmentId: 'seg-001', name: 'Premium Customers' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('updated'));
    assert.ok(result.segment);
  });

  it('evaluate_segment_membership returns membership result', async () => {
    const result = await byName['evaluate_segment_membership'].handler({
      params: { segmentId: 'seg-001', customerId: 'cust-1' },
      commerce,
    });
    assert.equal(result.success, true);
    assert.equal(result.segmentId, 'seg-001');
    assert.equal(result.customerId, 'cust-1');
    assert.equal(result.isMember, true);
    assert.ok(Array.isArray(result.matchedConditions));
    assert.ok(result.evaluatedAt);
  });

  it('rebuild_dynamic_segment returns rebuild stats', async () => {
    const result = await byName['rebuild_dynamic_segment'].handler({
      params: { segmentId: 'seg-001' },
      allowApply: true,
      commerce,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('rebuilt'));
    assert.equal(result.segmentId, 'seg-001');
    assert.equal(result.memberCount, 130);
    assert.equal(result.added, 5);
    assert.equal(result.removed, 3);
    assert.ok(result.evaluatedAt);
  });
});
