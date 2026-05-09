// Unit tests for the plan-resolver cluster extracted from mcp-server.js.

import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';

import {
  AGENTIC_PLAN_PARAM_TEMPLATE,
  AGENTIC_SLA_LEVELS,
  MAX_PLAN_STEPS,
  getByPath,
  normalizeSlaLevel,
  resolveAgenticPlanPath,
  resolveAgenticPlanValue,
} from '../../src/mcp/plan-resolver.js';

describe('plan-resolver · constants', () => {
  it('MAX_PLAN_STEPS is a positive integer', () => {
    assert.equal(typeof MAX_PLAN_STEPS, 'number');
    assert.ok(MAX_PLAN_STEPS > 0);
    assert.ok(Number.isInteger(MAX_PLAN_STEPS));
  });

  it('AGENTIC_SLA_LEVELS contains the expected canonical levels', () => {
    assert.deepEqual(AGENTIC_SLA_LEVELS, ['standard', 'expedited', 'critical']);
  });

  it('AGENTIC_PLAN_PARAM_TEMPLATE matches {{ … }} forms', () => {
    assert.ok(AGENTIC_PLAN_PARAM_TEMPLATE.test('{{ steps.0.id }}'));
    assert.ok(AGENTIC_PLAN_PARAM_TEMPLATE.test('{{steps.0.id}}'));
    // Pattern is intentionally loose on inner content (1+ non-`}` chars).
    // Whitespace-only inner text is matched by the regex but resolves to
    // undefined in resolveAgenticPlanPath, which is the real safety boundary.
    assert.ok(!AGENTIC_PLAN_PARAM_TEMPLATE.test('hello {{ x }} world'));
    assert.ok(!AGENTIC_PLAN_PARAM_TEMPLATE.test('not a template'));
    assert.ok(!AGENTIC_PLAN_PARAM_TEMPLATE.test('{{}}'));
  });
});

describe('plan-resolver · normalizeSlaLevel', () => {
  it('canonicalizes recognised levels', () => {
    assert.equal(normalizeSlaLevel('STANDARD'), 'standard');
    assert.equal(normalizeSlaLevel('  expedited  '), 'expedited');
    assert.equal(normalizeSlaLevel('Critical'), 'critical');
  });

  it('rejects unrecognised strings', () => {
    assert.equal(normalizeSlaLevel('urgent'), null);
    assert.equal(normalizeSlaLevel(''), null);
  });

  it('rejects non-strings', () => {
    assert.equal(normalizeSlaLevel(null), null);
    assert.equal(normalizeSlaLevel(undefined), null);
    assert.equal(normalizeSlaLevel(123), null);
  });
});

describe('plan-resolver · getByPath', () => {
  it('walks nested objects', () => {
    const v = { a: { b: { c: 42 } } };
    assert.equal(getByPath(v, ['a', 'b', 'c']), 42);
  });

  it('walks arrays via numeric segments', () => {
    const v = [{ x: 1 }, { x: 2 }];
    assert.equal(getByPath(v, ['1', 'x']), 2);
  });

  it('returns undefined when path bottoms out', () => {
    assert.equal(getByPath({ a: 1 }, ['a', 'missing']), undefined);
    assert.equal(getByPath(null, ['a']), undefined);
    assert.equal(getByPath('not an object', ['a']), undefined);
  });

  it('returns the input unchanged for empty path', () => {
    const v = { a: 1 };
    assert.equal(getByPath(v, []), v);
  });
});

describe('plan-resolver · resolveAgenticPlanPath', () => {
  const context = {
    steps: [
      { result: { orderId: 'ORD-1' } },
      { result: { paymentId: 'PAY-2' } },
    ],
    latest: { kind: 'order' },
    byTool: { create_order: { result: { orderId: 'ORD-9' } } },
    sla: { level: 'expedited', deadlineSeconds: 60 },
  };

  it('resolves steps.<idx>.<path>', () => {
    assert.equal(resolveAgenticPlanPath(context, 'steps.0.result.orderId'), 'ORD-1');
    assert.equal(resolveAgenticPlanPath(context, 'steps.1.result.paymentId'), 'PAY-2');
  });

  it('resolves steps.<idx>[<i>] bracket index syntax', () => {
    const ctx = { steps: [{ rows: ['a', 'b', 'c'] }] };
    assert.equal(resolveAgenticPlanPath(ctx, 'steps.0.rows[1]'), 'b');
  });

  it('resolves latest.<path>', () => {
    assert.equal(resolveAgenticPlanPath(context, 'latest.kind'), 'order');
  });

  it('resolves tool.<name>.<path>', () => {
    assert.equal(
      resolveAgenticPlanPath(context, 'tool.create_order.result.orderId'),
      'ORD-9'
    );
  });

  it('resolves sla.<path>', () => {
    assert.equal(resolveAgenticPlanPath(context, 'sla.level'), 'expedited');
    assert.equal(resolveAgenticPlanPath(context, 'sla.deadlineSeconds'), 60);
  });

  it('resolves the slaLevel shorthand', () => {
    assert.equal(resolveAgenticPlanPath(context, 'slaLevel'), 'expedited');
  });

  it('returns undefined for unknown roots and malformed paths', () => {
    assert.equal(resolveAgenticPlanPath(context, 'unknown.path'), undefined);
    assert.equal(resolveAgenticPlanPath(context, 'steps'), undefined); // no index
    assert.equal(resolveAgenticPlanPath(context, 'steps.99.result.id'), undefined);
    assert.equal(resolveAgenticPlanPath(context, 'steps.-1.result.id'), undefined);
    assert.equal(resolveAgenticPlanPath(context, 'tool'), undefined); // no name
    assert.equal(resolveAgenticPlanPath(context, ''), undefined);
  });

  it('returns undefined for null/non-string inputs', () => {
    assert.equal(resolveAgenticPlanPath(null, 'latest.kind'), undefined);
    assert.equal(resolveAgenticPlanPath(context, null), undefined);
    assert.equal(resolveAgenticPlanPath(context, 123), undefined);
  });
});

describe('plan-resolver · resolveAgenticPlanValue', () => {
  const context = {
    steps: [{ result: { orderId: 'ORD-1', total: 99.5 } }],
    latest: { kind: 'order' },
  };

  it('replaces template strings with resolved values', () => {
    const out = resolveAgenticPlanValue('{{ steps.0.result.orderId }}', context);
    assert.equal(out.value, 'ORD-1');
    assert.deepEqual(out.unresolved, []);
  });

  it('passes through non-template strings unchanged', () => {
    const out = resolveAgenticPlanValue('hello world', context);
    assert.equal(out.value, 'hello world');
    assert.deepEqual(out.unresolved, []);
  });

  it('substitutes templates inside objects, recording locations', () => {
    const input = {
      orderId: '{{ steps.0.result.orderId }}',
      amount: '{{ steps.0.result.total }}',
      memo: 'static',
    };
    const out = resolveAgenticPlanValue(input, context);
    assert.deepEqual(out.value, {
      orderId: 'ORD-1',
      amount: 99.5,
      memo: 'static',
    });
    assert.deepEqual(out.unresolved, []);
  });

  it('substitutes templates inside arrays', () => {
    const input = ['{{ steps.0.result.orderId }}', 'literal', 7];
    const out = resolveAgenticPlanValue(input, context);
    assert.deepEqual(out.value, ['ORD-1', 'literal', 7]);
  });

  it('records unresolved templates with breadcrumbs and substitutes null', () => {
    const input = {
      orderId: '{{ steps.0.result.orderId }}',
      missing: '{{ steps.0.result.does_not_exist }}',
      nested: {
        also_missing: '{{ steps.99.result.x }}',
      },
    };
    const out = resolveAgenticPlanValue(input, context);
    assert.equal(out.value.orderId, 'ORD-1');
    assert.equal(out.value.missing, null);
    assert.equal(out.value.nested.also_missing, null);
    assert.equal(out.unresolved.length, 2);
    assert.ok(out.unresolved[0].includes('steps.0.result.does_not_exist'));
    assert.ok(out.unresolved[1].includes('steps.99.result.x'));
  });

  it('passes through Date / Buffer / Map / Set unchanged', () => {
    const d = new Date('2026-01-01T00:00:00.000Z');
    const b = Buffer.from('x');
    const m = new Map([['k', 'v']]);
    const s = new Set(['a']);
    assert.equal(resolveAgenticPlanValue(d, context).value, d);
    assert.equal(resolveAgenticPlanValue(b, context).value, b);
    assert.equal(resolveAgenticPlanValue(m, context).value, m);
    assert.equal(resolveAgenticPlanValue(s, context).value, s);
  });

  it('passes through primitives and null/undefined unchanged', () => {
    assert.equal(resolveAgenticPlanValue(42, context).value, 42);
    assert.equal(resolveAgenticPlanValue(true, context).value, true);
    assert.equal(resolveAgenticPlanValue(null, context).value, null);
    assert.equal(resolveAgenticPlanValue(undefined, context).value, undefined);
  });
});
