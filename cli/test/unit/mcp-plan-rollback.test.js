// Unit tests for cli/src/mcp/plan-rollback.js
//
// Covers `createRunPlanRollback`:
//  - returns null when rollback does not apply (dry run, disabled, or the
//    plan did not fail)
//  - only steps with compensation hints are candidates, processed in
//    reverse order
//  - the first successful compensation wins; failures fall through
//  - steps whose compensation params can't be derived are reported
//  - each compensation is written to the replay log with phase=rollback
//  - `fullyReverted` reflects every rollback step succeeding

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createRunPlanRollback } from '../../src/mcp/plan-rollback.js';
import { createCostSummary } from '../../src/mcp/cost-budget.js';

function makeRollback({ stepResult } = {}) {
  const executed = [];
  const events = [];
  const runPlanRollback = createRunPlanRollback({
    toolDefsByName: new Map([
      ['cancel_order', {}],
      ['release_reservation', {}],
    ]),
    inferPolicyDomain: () => 'orders',
    executeToolStepInPlan: async (input) => {
      executed.push(input);
      return (
        stepResult?.(input) ?? {
          index: input.stepIndex,
          tool: input.toolName,
          status: 'rollback_success',
          elapsedMs: 1,
          charge: null,
        }
      );
    },
    addAgenticReplayEvent: async (event) => {
      events.push(event);
    },
  });
  return { runPlanRollback, executed, events };
}

const baseInput = {
  dryRun: false,
  rollbackOnFailure: true,
  finalStatus: 'failed',
  costSummary: createCostSummary('execute'),
  executionRequestId: 'req-1',
  executionSessionId: 'sess-1',
  planSignature: 'plan-sig',
  normalizedSlaLevel: 'standard',
};

const completedSteps = [
  {
    step: { index: 0, tool: 'create_order', params: { customerId: 'c1' } },
    outcome: { result: { order: { id: 'ord_1' } } },
  },
  {
    step: { index: 1, tool: 'list_orders', params: {} },
    outcome: { result: [] },
  },
  {
    step: { index: 2, tool: 'reserve_inventory', params: {} },
    outcome: { result: { reservation: { id: 'res_1' } } },
  },
];

describe('createRunPlanRollback', () => {
  it('returns null when rollback does not apply', async () => {
    const { runPlanRollback, executed } = makeRollback();
    for (const patch of [
      { dryRun: true },
      { rollbackOnFailure: false },
      { finalStatus: 'success' },
    ]) {
      const out = await runPlanRollback({
        ...baseInput,
        ...patch,
        executedForRollback: completedSteps,
      });
      assert.equal(out, null);
    }
    assert.equal(executed.length, 0);
  });

  it('compensates candidate steps in reverse order and logs each attempt', async () => {
    const { runPlanRollback, executed, events } = makeRollback();
    const out = await runPlanRollback({ ...baseInput, executedForRollback: completedSteps });

    // list_orders has no compensation hint → not a candidate.
    assert.equal(out.attempted, 2);
    assert.equal(out.fullyReverted, true);
    assert.deepEqual(
      executed.map((e) => e.toolName),
      ['release_reservation', 'cancel_order'],
    );
    assert.deepEqual(executed[1].params, { orderId: 'ord_1' });
    assert.equal(executed[1].isRollback, true);
    assert.equal(executed[1].dryRun, false);
    assert.equal(executed[1].requestId, 'req-1');

    assert.equal(out.steps.length, 2);
    assert.equal(out.steps[1].source, 'create_order');
    assert.deepEqual(out.steps[1].compensationTools, ['cancel_order']);
    assert.deepEqual(out.steps[1].compensationParams, { orderId: 'ord_1' });

    assert.equal(events.length, 2);
    const evt = events[1];
    assert.equal(evt.tool, 'agentic_execute_plan');
    assert.equal(evt.status, 'rollback_success');
    assert.equal(evt.planSignature, 'plan-sig');
    assert.equal(evt.source, 'agentic_execute_plan');
    assert.equal(evt.agentic, true);
    assert.deepEqual(evt.notes, {
      phase: 'rollback',
      compensated: true,
      slaLevel: 'standard',
      index: 0,
      source: 'create_order',
    });
    assert.match(evt.eventId, /^[0-9a-f-]{36}$/);
  });

  it('reports rollback_failed when no compensation params can be derived', async () => {
    const { runPlanRollback, executed, events } = makeRollback();
    const out = await runPlanRollback({
      ...baseInput,
      executedForRollback: [
        { step: { index: 0, tool: 'create_order', params: {} }, outcome: { result: {} } },
      ],
    });
    assert.equal(executed.length, 0);
    assert.equal(out.fullyReverted, false);
    assert.equal(out.steps[0].status, 'rollback_failed');
    assert.equal(out.steps[0].reason, 'No compensation parameters');
    assert.equal(events[0].notes.compensated, false);
  });

  it('records failed compensations in the cost summary and marks fullyReverted false', async () => {
    const { runPlanRollback } = makeRollback({
      stepResult: (input) => ({
        index: input.stepIndex,
        tool: input.toolName,
        status: 'rollback_failed',
        error: 'nope',
        charge: {
          charged: false,
          blocked: true,
          reason: 'blocked',
          rule: { chainId: 'base', tokenSymbol: 'USDC', amount: '1' },
        },
      }),
    });
    const costSummary = createCostSummary('execute');
    const out = await runPlanRollback({
      ...baseInput,
      costSummary,
      executedForRollback: [completedSteps[0]],
    });
    assert.equal(out.fullyReverted, false);
    assert.equal(out.steps[0].status, 'rollback_failed');
    assert.equal(costSummary.totalEntries, 1);
    assert.equal(costSummary.blockedEntries, 1);
    assert.equal(costSummary.entries[0].source, 'rollback');
  });
});
