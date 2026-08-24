// Unit tests for cli/src/mcp/plan-executor.js
//
// Covers `createExecuteAgenticPlan`:
//  - step-limit guard
//  - happy path: per-step + final replay events, signatures, cost summary,
//    template resolution between steps, routing/stepSignature decoration
//  - unresolved references produce an invalid step without executing
//  - cost budget pre-check blocks a step before execution
//  - stopOnFailure halts the loop; dry runs stop on non-dry_run_success
//  - rollback is delegated and surfaced in the result + final event

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createExecuteAgenticPlan } from '../../src/mcp/plan-executor.js';
import { MAX_PLAN_STEPS } from '../../src/mcp/plan-resolver.js';

function makeExecutor(overrides = {}) {
  const executed = [];
  const events = [];
  const rollbackCalls = [];
  const executeAgenticPlan = createExecuteAgenticPlan({
    inferPolicyDomain: () => 'orders',
    getToolRuntimeMeta: (name) => ({
      name,
      permission: 'write',
      policyDomain: 'orders',
      sideEffect: 'write',
      compensations: [],
      idempotent: false,
    }),
    buildPlanStepRouting: ({ tool }) => ({
      slaLevel: null,
      primary: { agent: `agent:${tool}`, score: 1, confidence: 1, level: 'high' },
      alternatives: [],
      ambiguous: false,
    }),
    getAgenticToolPricing: async () => null,
    executeToolStepInPlan: async (input) => {
      executed.push(input);
      return (
        overrides.stepResult?.(input) ?? {
          index: input.stepIndex,
          tool: input.toolName,
          status: input.dryRun ? 'dry_run_success' : 'success',
          elapsedMs: 2,
          policy: { allowed: true, domain: 'orders' },
          permission: { allowed: true },
          charge: { charged: false, blocked: false, rule: null },
          result: { id: `${input.toolName}-id` },
          error: null,
        }
      );
    },
    addAgenticReplayEvent: async (event) => {
      events.push(event);
    },
    runPlanRollback: async (input) => {
      rollbackCalls.push(input);
      return overrides.rollback ?? null;
    },
    ...overrides.deps,
  });
  return { executeAgenticPlan, executed, events, rollbackCalls };
}

describe('createExecuteAgenticPlan', () => {
  it('rejects plans over MAX_PLAN_STEPS', async () => {
    const { executeAgenticPlan, executed } = makeExecutor();
    const out = await executeAgenticPlan({
      steps: Array.from({ length: MAX_PLAN_STEPS + 1 }, () => ({ tool: 'x' })),
      requestId: 'req',
    });
    assert.equal(out.finalStatus, 'failed');
    assert.equal(out.tool, 'agentic_execute_plan');
    assert.equal(out.steps[0].status, 'invalid');
    assert.equal(out.planSignature, null);
    assert.equal(executed.length, 0);
  });

  it('executes steps in order, resolves templates, and logs per-step + final events', async () => {
    const { executeAgenticPlan, executed, events, rollbackCalls } = makeExecutor();
    const out = await executeAgenticPlan({
      steps: [
        { tool: 'create_order', params: { customerId: 'c1' } },
        { tool: 'ship_order', params: { orderId: '{{ steps.0.result.id }}' } },
      ],
      dryRun: false,
      requestId: 'req-1',
      sessionId: 'sess-1',
      slaLevel: 'standard',
    });

    assert.equal(out.finalStatus, 'success');
    assert.equal(out.completedSteps, 2);
    assert.equal(out.failedSteps, 0);
    assert.equal(out.requestId, 'req-1');
    assert.equal(out.sessionId, 'sess-1');
    assert.match(out.planSignature, /^[0-9a-f]{64}$/);
    assert.match(out.executionSignature, /^[0-9a-f]{64}$/);
    assert.equal(out.rollback, null);
    assert.equal(out.costSummary.mode, 'execute');

    assert.deepEqual(executed[1].params, { orderId: 'create_order-id' });
    assert.equal(executed[1].requestId, 'req-1');
    assert.equal(out.steps[1].routing.primary.agent, 'agent:ship_order');
    assert.match(out.steps[1].stepSignature, /^[0-9a-f]{64}$/);
    assert.deepEqual(out.steps[0].rollbackTarget, ['cancel_order']);

    assert.equal(events.length, 3);
    assert.equal(events[0].tool, 'agentic_execute_plan');
    assert.equal(events[0].status, 'success');
    assert.equal(events[0].source, 'agentic_execute_plan');
    assert.equal(events[0].notes.executedBy, 'agentic_execute_plan');
    assert.equal(events[0].notes.index, 0);
    const final = events[2];
    assert.equal(final.notes.final, true);
    assert.equal(final.policyDomain, 'agentic');
    assert.equal(final.notes.planSignature, out.planSignature);
    assert.equal(final.executionSignature, out.executionSignature);
    assert.deepEqual(final.result.stepStatuses, ['success', 'success']);

    // Rollback is consulted exactly once with the run summary.
    assert.equal(rollbackCalls.length, 1);
    assert.equal(rollbackCalls[0].finalStatus, 'success');
    assert.equal(rollbackCalls[0].executedForRollback.length, 2);
    assert.equal(rollbackCalls[0].planSignature, out.planSignature);
  });

  it('generates request/session ids when omitted', async () => {
    const { executeAgenticPlan } = makeExecutor();
    const out = await executeAgenticPlan({ steps: [{ tool: 'create_order' }] });
    assert.match(out.requestId, /^[0-9a-f-]{36}$/);
    assert.equal(out.sessionId, out.requestId);
  });

  it('marks unresolved references invalid without executing the step', async () => {
    const { executeAgenticPlan, executed } = makeExecutor();
    const out = await executeAgenticPlan({
      steps: [{ tool: 'ship_order', params: { orderId: '{{ steps.4.result.id }}' } }],
      dryRun: false,
    });
    assert.equal(executed.length, 0);
    assert.equal(out.steps[0].status, 'invalid');
    assert.match(out.steps[0].error, /Unresolved plan parameter reference/);
    assert.equal(out.finalStatus, 'success'); // 'invalid' is not in the failure set
    assert.equal(out.failedSteps, 1);
  });

  it('blocks a step on the cost budget before execution', async () => {
    const { executeAgenticPlan, executed } = makeExecutor({
      deps: {
        getAgenticToolPricing: async () => ({ chainId: 'base', tokenSymbol: 'USDC', amount: '2' }),
      },
    });
    const out = await executeAgenticPlan({
      steps: [{ tool: 'create_order' }],
      dryRun: false,
      costBudget: { USDC: 1 },
    });
    assert.equal(executed.length, 0);
    assert.equal(out.steps[0].status, 'treasury_block');
    assert.equal(out.budgetExceeded, true);
    assert.equal(out.budgetViolations[0].projectedTotal, 2);
    assert.equal(out.steps[0].charge.rule.budgetLimit, 1);
    assert.equal(out.finalStatus, 'failed');
    assert.equal(out.costSummary.blockedEntries, 1);
  });

  it('stops on the first failure when stopOnFailure is set and surfaces rollback', async () => {
    const rollback = { attempted: 1, steps: [], fullyReverted: true };
    const { executeAgenticPlan, executed, events } = makeExecutor({
      rollback,
      stepResult: (input) =>
        input.toolName === 'ship_order'
          ? {
              index: input.stepIndex,
              tool: input.toolName,
              status: 'error',
              error: 'x',
              charge: null,
              result: null,
            }
          : undefined,
    });
    const out = await executeAgenticPlan({
      steps: [{ tool: 'create_order' }, { tool: 'ship_order' }, { tool: 'list_orders' }],
      dryRun: false,
    });
    assert.deepEqual(
      executed.map((e) => e.toolName),
      ['create_order', 'ship_order'],
    );
    assert.equal(out.finalStatus, 'failed');
    assert.equal(out.rollback, rollback);
    assert.deepEqual(events.at(-1).notes.rollback, { attempted: 1, fullyReverted: true });
  });

  it('continues past failures when stopOnFailure is false', async () => {
    const { executeAgenticPlan, executed } = makeExecutor({
      stepResult: (input) =>
        input.toolName === 'ship_order'
          ? {
              index: input.stepIndex,
              tool: input.toolName,
              status: 'error',
              error: 'x',
              charge: null,
              result: null,
            }
          : undefined,
    });
    await executeAgenticPlan({
      steps: [{ tool: 'ship_order' }, { tool: 'list_orders' }],
      dryRun: false,
      stopOnFailure: false,
    });
    assert.equal(executed.length, 2);
  });

  it('reports dry_run as the final status for dry runs', async () => {
    const { executeAgenticPlan } = makeExecutor();
    const out = await executeAgenticPlan({ steps: [{ tool: 'create_order' }], dryRun: true });
    assert.equal(out.finalStatus, 'dry_run');
    assert.equal(out.completedSteps, 1);
  });
});
