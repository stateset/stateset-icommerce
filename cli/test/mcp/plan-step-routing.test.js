// Unit tests for cli/src/mcp/plan-step-routing.js
//
// Covers `buildPlanStepRouting`:
//  - Constructs the routing intent string from tool + params (params are
//    JCS-canonicalized via the shared compactReplayValue + stableStringify
//    helpers before stringification).
//  - Passes the normalized SLA level through to the router.
//  - Maps routing.primary fields one-for-one into the response.
//  - Falls back to the `customer-service` default when the router returns
//    no primary candidate.
//  - Maps each alternative through the same shape filter.
//  - Coerces ambiguous to a strict boolean.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { buildPlanStepRouting } from '../../src/mcp/plan-step-routing.js';

function makeRouter(returnValue, capture = {}) {
  capture.calls = [];
  return (intent, opts) => {
    capture.calls.push({ intent, opts });
    return returnValue;
  };
}

describe('buildPlanStepRouting', () => {
  it('passes a normalized intent string and SLA level to the router', () => {
    const capture = {};
    const route = makeRouter(
      {
        routingContext: { slaLevel: 'standard' },
        primary: { agent: 'orders', score: 0.9, confidence: 0.8, level: 'high' },
      },
      capture,
    );

    buildPlanStepRouting(
      { tool: 'create_order', params: { customerId: 'c1' }, slaLevel: 'STANDARD' },
      route,
    );

    assert.equal(capture.calls.length, 1);
    const { intent, opts } = capture.calls[0];
    // Tool name is rewritten with spaces (underscores → spaces) for routing.
    assert.match(intent, /^create order /);
    // Params are JCS-stringified, so the customerId appears verbatim.
    assert.match(intent, /"customerId":"c1"/);
    // SLA level is normalized to lowercase.
    assert.equal(opts.slaLevel, 'standard');
  });

  it('passes undefined for slaLevel when normalization yields a falsy value', () => {
    const capture = {};
    const route = makeRouter(
      { primary: { agent: 'a', score: 0, confidence: 0, level: 'l' } },
      capture,
    );

    buildPlanStepRouting({ tool: 'list_orders' }, route);

    // Without an SLA on the step, the call passes opts.slaLevel === undefined.
    assert.equal(capture.calls[0].opts.slaLevel, undefined);
  });

  it('handles missing tool gracefully (empty intent prefix)', () => {
    const capture = {};
    const route = makeRouter(
      { primary: { agent: 'a', score: 0, confidence: 0, level: 'l' } },
      capture,
    );

    buildPlanStepRouting({ params: { x: 1 } }, route);

    // Intent starts with " " (the empty tool name + the space + params).
    assert.match(capture.calls[0].intent, /^ /);
  });

  it('maps routing.primary fields one-for-one', () => {
    const route = () => ({
      routingContext: { slaLevel: 'expedited' },
      primary: {
        agent: 'fulfillment',
        score: 0.95,
        confidence: 0.85,
        level: 'high',
      },
      alternatives: [],
    });

    const out = buildPlanStepRouting({ tool: 'ship_order' }, route);

    assert.equal(out.slaLevel, 'expedited');
    assert.deepEqual(out.primary, {
      agent: 'fulfillment',
      score: 0.95,
      confidence: 0.85,
      level: 'high',
    });
  });

  it('falls back to the customer-service default when primary is missing', () => {
    const route = () => ({ alternatives: [] });

    const out = buildPlanStepRouting({ tool: 'unknown_tool' }, route);

    assert.deepEqual(out.primary, {
      agent: 'customer-service',
      score: 0,
      confidence: 0,
      level: 'default',
    });
  });

  it('maps alternatives through the same shape filter', () => {
    const route = () => ({
      primary: { agent: 'a', score: 1, confidence: 1, level: 'high' },
      alternatives: [
        { agent: 'b', score: 0.6, confidence: 0.5, level: 'medium', extraField: 'dropped' },
        { agent: 'c', score: 0.3, confidence: 0.2, level: 'low' },
      ],
    });

    const out = buildPlanStepRouting({ tool: 'x' }, route);

    assert.equal(out.alternatives.length, 2);
    // The `extraField` key is intentionally dropped — output keeps only
    // the four fields the orchestrator cares about.
    assert.deepEqual(out.alternatives[0], {
      agent: 'b',
      score: 0.6,
      confidence: 0.5,
      level: 'medium',
    });
    assert.deepEqual(out.alternatives[1], {
      agent: 'c',
      score: 0.3,
      confidence: 0.2,
      level: 'low',
    });
  });

  it('returns alternatives = [] when the router returns no list or non-array', () => {
    const out1 = buildPlanStepRouting(
      { tool: 'x' },
      () => ({ primary: { agent: 'a', score: 0, confidence: 0, level: 'l' } }),
    );
    assert.deepEqual(out1.alternatives, []);

    const out2 = buildPlanStepRouting(
      { tool: 'x' },
      () => ({
        primary: { agent: 'a', score: 0, confidence: 0, level: 'l' },
        alternatives: 'not an array',
      }),
    );
    assert.deepEqual(out2.alternatives, []);
  });

  it('coerces routing.ambiguous to a strict boolean', () => {
    const truthyRoute = () => ({
      primary: { agent: 'a', score: 0, confidence: 0, level: 'l' },
      ambiguous: 'truthy-but-not-a-bool',
    });
    assert.equal(buildPlanStepRouting({ tool: 'x' }, truthyRoute).ambiguous, true);

    const falsyRoute = () => ({
      primary: { agent: 'a', score: 0, confidence: 0, level: 'l' },
      ambiguous: 0,
    });
    assert.equal(buildPlanStepRouting({ tool: 'x' }, falsyRoute).ambiguous, false);

    const missingRoute = () => ({
      primary: { agent: 'a', score: 0, confidence: 0, level: 'l' },
    });
    assert.equal(buildPlanStepRouting({ tool: 'x' }, missingRoute).ambiguous, false);
  });

  it('returns slaLevel = null when the router omits routingContext', () => {
    const route = () => ({
      primary: { agent: 'a', score: 0, confidence: 0, level: 'l' },
    });
    assert.equal(buildPlanStepRouting({ tool: 'x' }, route).slaLevel, null);
  });

  it('JCS-canonicalizes params so key order does not affect the intent', () => {
    const capture1 = {};
    const route1 = makeRouter(
      { primary: { agent: 'a', score: 0, confidence: 0, level: 'l' } },
      capture1,
    );
    buildPlanStepRouting(
      { tool: 'create_order', params: { b: 2, a: 1 } },
      route1,
    );

    const capture2 = {};
    const route2 = makeRouter(
      { primary: { agent: 'a', score: 0, confidence: 0, level: 'l' } },
      capture2,
    );
    buildPlanStepRouting(
      { tool: 'create_order', params: { a: 1, b: 2 } },
      route2,
    );

    // Identical intents regardless of insertion order — that's the point
    // of running params through compactReplayValue + stableStringify.
    assert.equal(capture1.calls[0].intent, capture2.calls[0].intent);
  });
});
