// Unit tests for cli/src/mcp/plan-simulator.js
//
// Covers `createSimulateAgenticPlan`:
//  - step-limit guard produces a single invalid outcome
//  - missing tool / unknown tool / unresolved template refs mark the plan
//    non-executable but keep walking
//  - policy and permission blocks map to their statuses
//  - later steps can reference earlier steps' simulated results
//  - treasury pricing feeds the cost summary, and the cost budget blocks
//    once the projected total is exceeded
//  - a deterministic plan signature is produced

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createSimulateAgenticPlan } from '../../src/mcp/plan-simulator.js';
import { MAX_PLAN_STEPS } from '../../src/mcp/plan-resolver.js';

const KNOWN = new Set(['create_order', 'cancel_order', 'list_orders']);

function makeSimulator(overrides = {}) {
  const calls = { policy: [], permission: [] };
  const simulate = createSimulateAgenticPlan({
    inferPolicyDomain: () => 'orders',
    buildPlanStepRouting: ({ tool }) => ({
      slaLevel: null,
      primary: { agent: `agent-for-${tool}`, score: 1, confidence: 1, level: 'high' },
      alternatives: [],
      ambiguous: false,
    }),
    getToolRuntimeMeta: (name) => ({
      name,
      permission: KNOWN.has(name) ? 'write' : 'unknown',
      policyDomain: 'orders',
      sideEffect: 'write',
      compensations: [],
      idempotent: false,
    }),
    evaluatePolicy: async (tool, params, extra, domain) => {
      calls.policy.push({ tool, params, extra, domain });
      return { allowed: true, params, domain, actions: [] };
    },
    checkPermission: async (tool, params) => {
      calls.permission.push({ tool, params });
      return { allowed: true };
    },
    getAgenticToolPricing: async () => null,
    ...overrides,
  });
  return { simulate, calls };
}

describe('createSimulateAgenticPlan', () => {
  it('rejects plans over MAX_PLAN_STEPS with a single invalid outcome', async () => {
    const { simulate } = makeSimulator();
    const steps = Array.from({ length: MAX_PLAN_STEPS + 1 }, () => ({ tool: 'list_orders' }));
    const out = await simulate({ steps });
    assert.equal(out.executable, false);
    assert.equal(out.totalSteps, MAX_PLAN_STEPS + 1);
    assert.equal(out.failedSteps, 1);
    assert.equal(out.outcomes.length, 1);
    assert.equal(out.outcomes[0].status, 'invalid');
    assert.match(out.outcomes[0].error, /at most/);
    assert.equal(out.planSignature, null);
  });

  it('simulates a healthy plan and threads results into later steps', async () => {
    const { simulate, calls } = makeSimulator();
    const out = await simulate({
      steps: [
        { tool: 'create_order', params: { customerId: 'c1' } },
        { tool: 'list_orders', params: { after: '{{ steps.0.status }}' } },
      ],
      slaLevel: 'EXPEDITED',
    });
    assert.equal(out.executable, true);
    assert.equal(out.tool, 'agentic_plan');
    assert.equal(out.failedSteps, 0);
    assert.equal(out.slaLevel, 'expedited');
    assert.equal(out.outcomes[0].status, 'success');
    assert.equal(out.outcomes[0].routing.primary.agent, 'agent-for-create_order');
    assert.equal(out.outcomes[0].simulation, true);
    assert.equal(out.outcomes[0].treasury, null);
    assert.match(out.outcomes[0].stepSignature, /^[0-9a-f]{64}$/);
    assert.match(out.outcomes[0].replay.paramsHash, /^[0-9a-f]{64}$/);
    assert.equal(out.outcomes[0].mutationManifest?.phase ?? 'simulate', 'simulate');
    // Step 1 resolved `{{ steps.0.status }}` → 'success'.
    assert.deepEqual(out.outcomes[1].params, { after: 'success' });
    assert.deepEqual(calls.permission[1].params, { after: 'success' });
    assert.deepEqual(calls.policy[0].extra, {
      requestId: 'agentic_plan',
      sessionId: 'agentic_plan',
    });
    assert.match(out.planSignature, /^[0-9a-f]{64}$/);
    assert.equal(out.costSummary.mode, 'simulate');
  });

  it('flags missing, unknown, and unresolved steps as invalid', async () => {
    const { simulate } = makeSimulator();
    const out = await simulate({
      steps: [
        {},
        { tool: 'nope' },
        { tool: 'list_orders', params: { id: '{{ steps.7.result.id }}' } },
      ],
    });
    assert.equal(out.executable, false);
    assert.equal(out.failedSteps, 3);
    assert.equal(out.outcomes[0].error, 'Step.tool is required');
    assert.equal(out.outcomes[1].error, "Unknown tool 'nope'");
    assert.match(out.outcomes[2].error, /Unresolved plan parameter reference/);
    assert.deepEqual(out.outcomes[2].notes.availableContext, {
      latestStep: 1,
      stepsAvailable: 2,
    });
  });

  it('maps policy and permission decisions to statuses', async () => {
    const { simulate } = makeSimulator({
      evaluatePolicy: async (tool, params) =>
        tool === 'create_order'
          ? { allowed: false, params, domain: 'orders', reason: 'nope' }
          : { allowed: true, params, domain: 'orders' },
      checkPermission: async (tool) =>
        tool === 'cancel_order'
          ? { allowed: false, preview: true, reason: 'preview only' }
          : { allowed: false, preview: false, reason: 'denied' },
    });
    const out = await simulate({
      steps: [{ tool: 'create_order' }, { tool: 'cancel_order' }, { tool: 'list_orders' }],
    });
    assert.deepEqual(
      out.outcomes.map((o) => o.status),
      ['policy_block', 'preview', 'permission_block'],
    );
    assert.equal(out.outcomes[0].policy.reason, 'nope');
    assert.equal(out.outcomes[1].permission.reason, 'preview only');
    assert.equal(out.executable, false);
  });

  it('accumulates treasury cost and blocks once the budget is exceeded', async () => {
    const { simulate } = makeSimulator({
      getAgenticToolPricing: async () => ({ chainId: 'base', tokenSymbol: 'USDC', amount: '0.6' }),
    });
    const out = await simulate({
      steps: [{ tool: 'create_order' }, { tool: 'cancel_order' }],
      costBudget: { usdc: 1 },
    });
    assert.equal(out.outcomes[0].status, 'success');
    assert.deepEqual(out.outcomes[0].treasury, {
      required: true,
      chainId: 'base',
      tokenSymbol: 'USDC',
      amount: '0.6',
    });
    assert.equal(out.outcomes[1].status, 'treasury_block');
    assert.equal(out.budgetExceeded, true);
    assert.equal(out.executable, false);
    assert.deepEqual(out.costBudget, { USDC: 1 });
    assert.equal(out.budgetViolations.length, 1);
    assert.equal(out.budgetViolations[0].step, 1);
    assert.equal(out.budgetViolations[0].projectedTotal, 1.2);
    assert.match(out.outcomes[1].error, /Cost budget exceeded for base:USDC/);
    assert.equal(out.costSummary.totalEntries, 2);
    assert.equal(out.costSummary.blockedEntries, 1);
  });
});
