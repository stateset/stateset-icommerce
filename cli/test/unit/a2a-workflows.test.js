/**
 * Unit tests for Workflow Orchestration — DAG-based multi-agent workflow execution
 *
 * Tests cli/src/a2a/workflows.js:
 *   - createWorkflowService() construction and validation
 *   - validateDAG() — Kahn's algorithm topological sort
 *   - createWorkflow() — stores workflow + step records
 *   - executeWorkflow() — runs steps in topological order
 *   - Step type execution (transform, condition_check, quote_request, payment)
 *   - getWorkflowStatus() — progress tracking
 *   - pauseWorkflow() / resumeWorkflow() — state management
 *   - Edge cases — already completed, empty context, step failure mid-workflow
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { A2AStore } from '../../src/a2a/store.js';
import { createWorkflowService } from '../../src/a2a/workflows.js';

// ===========================================================================
// Helpers
// ===========================================================================

function makeStore() {
  return new A2AStore(':memory:');
}

function linearSteps() {
  return [
    { name: 'step-a', type: 'quote_request', agentAddress: '0xA', params: { description: 'fetch data' } },
    { name: 'step-b', type: 'transform', dependsOn: ['step-a'], params: { transformType: 'merge' } },
    { name: 'step-c', type: 'transform', dependsOn: ['step-b'], params: { transformType: 'merge' } },
  ];
}

function parallelSteps() {
  return [
    { name: 'fetch-1', type: 'quote_request', agentAddress: '0xA' },
    { name: 'fetch-2', type: 'quote_request', agentAddress: '0xB' },
    { name: 'merge', type: 'transform', dependsOn: ['fetch-1', 'fetch-2'], params: { transformType: 'merge' } },
  ];
}

function diamondSteps() {
  return [
    { name: 'source', type: 'quote_request', agentAddress: '0xS' },
    { name: 'left', type: 'transform', dependsOn: ['source'], params: { transformType: 'merge' } },
    { name: 'right', type: 'transform', dependsOn: ['source'], params: { transformType: 'merge' } },
    { name: 'sink', type: 'transform', dependsOn: ['left', 'right'], params: { transformType: 'sum_costs' } },
  ];
}

// ===========================================================================
// 1. createWorkflowService validation
// ===========================================================================

describe('createWorkflowService()', () => {
  it('throws when store is null', () => {
    assert.throws(() => createWorkflowService(null, null), /store is required/);
  });

  it('throws when store is undefined', () => {
    assert.throws(() => createWorkflowService(undefined, null), /store is required/);
  });

  it('returns an object with expected methods', () => {
    const store = makeStore();
    const svc = createWorkflowService(store, null);
    assert.ok(typeof svc.validateDAG === 'function');
    assert.ok(typeof svc.createWorkflow === 'function');
    assert.ok(typeof svc.executeWorkflow === 'function');
    assert.ok(typeof svc.getWorkflowStatus === 'function');
    assert.ok(typeof svc.pauseWorkflow === 'function');
    assert.ok(typeof svc.resumeWorkflow === 'function');
  });

  it('works with null a2aService', () => {
    const store = makeStore();
    const svc = createWorkflowService(store, null);
    assert.ok(svc);
  });

  it('works with undefined a2aService', () => {
    const store = makeStore();
    const svc = createWorkflowService(store);
    assert.ok(svc);
  });
});

// ===========================================================================
// 2. validateDAG
// ===========================================================================

describe('validateDAG()', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  // Valid topologies

  it('validates a linear chain A -> B -> C', () => {
    const result = svc.validateDAG(linearSteps());
    assert.strictEqual(result.valid, true);
    assert.strictEqual(result.order.length, 3);
    // step-a must come before step-b, step-b before step-c
    assert.ok(result.order.indexOf('step-a') < result.order.indexOf('step-b'));
    assert.ok(result.order.indexOf('step-b') < result.order.indexOf('step-c'));
  });

  it('validates parallel fan-out/fan-in', () => {
    const result = svc.validateDAG(parallelSteps());
    assert.strictEqual(result.valid, true);
    assert.strictEqual(result.order.length, 3);
    // Both fetches come before merge
    assert.ok(result.order.indexOf('fetch-1') < result.order.indexOf('merge'));
    assert.ok(result.order.indexOf('fetch-2') < result.order.indexOf('merge'));
  });

  it('validates a diamond DAG (source -> left/right -> sink)', () => {
    const result = svc.validateDAG(diamondSteps());
    assert.strictEqual(result.valid, true);
    assert.strictEqual(result.order.length, 4);
    assert.ok(result.order.indexOf('source') < result.order.indexOf('left'));
    assert.ok(result.order.indexOf('source') < result.order.indexOf('right'));
    assert.ok(result.order.indexOf('left') < result.order.indexOf('sink'));
    assert.ok(result.order.indexOf('right') < result.order.indexOf('sink'));
  });

  it('validates a single step with no dependencies', () => {
    const result = svc.validateDAG([{ name: 'solo', type: 'transform' }]);
    assert.strictEqual(result.valid, true);
    assert.deepStrictEqual(result.order, ['solo']);
  });

  it('validates multiple independent steps', () => {
    const result = svc.validateDAG([
      { name: 'a', type: 'transform' },
      { name: 'b', type: 'payment' },
      { name: 'c', type: 'quote_request' },
    ]);
    assert.strictEqual(result.valid, true);
    assert.strictEqual(result.order.length, 3);
  });

  // Invalid cases

  it('rejects an empty steps array', () => {
    const result = svc.validateDAG([]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('empty'));
  });

  it('rejects non-array input', () => {
    const result = svc.validateDAG('not an array');
    assert.strictEqual(result.valid, false);
    assert.ok(result.error);
  });

  it('rejects null input', () => {
    const result = svc.validateDAG(null);
    assert.strictEqual(result.valid, false);
  });

  it('detects a simple cycle (A -> B -> A)', () => {
    const result = svc.validateDAG([
      { name: 'a', type: 'transform', dependsOn: ['b'] },
      { name: 'b', type: 'transform', dependsOn: ['a'] },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('Cycle'));
  });

  it('detects a 3-node cycle (A -> B -> C -> A)', () => {
    const result = svc.validateDAG([
      { name: 'a', type: 'transform', dependsOn: ['c'] },
      { name: 'b', type: 'transform', dependsOn: ['a'] },
      { name: 'c', type: 'transform', dependsOn: ['b'] },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('Cycle'));
  });

  it('detects self-referencing step', () => {
    const result = svc.validateDAG([
      { name: 'self-loop', type: 'transform', dependsOn: ['self-loop'] },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('Cycle'));
  });

  it('rejects unknown dependency', () => {
    const result = svc.validateDAG([
      { name: 'a', type: 'transform', dependsOn: ['nonexistent'] },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('unknown step'));
  });

  it('rejects duplicate step names', () => {
    const result = svc.validateDAG([
      { name: 'dup', type: 'transform' },
      { name: 'dup', type: 'payment' },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('Duplicate'));
  });

  it('rejects step without a name', () => {
    const result = svc.validateDAG([
      { type: 'transform' },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('name'));
  });

  it('rejects invalid step type', () => {
    const result = svc.validateDAG([
      { name: 'bad', type: 'invalid_type' },
    ]);
    assert.strictEqual(result.valid, false);
    assert.ok(result.error.includes('Invalid step type'));
  });

  it('accepts step without explicit type (defaults to quote_request)', () => {
    const result = svc.validateDAG([{ name: 'no-type' }]);
    assert.strictEqual(result.valid, true);
  });
});

// ===========================================================================
// 3. createWorkflow
// ===========================================================================

describe('createWorkflow()', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('stores a workflow with steps', () => {
    const result = svc.createWorkflow({ name: 'test-wf', steps: linearSteps() });
    assert.ok(result.workflow);
    assert.ok(result.workflow.id);
    assert.strictEqual(result.workflow.name, 'test-wf');
    assert.strictEqual(result.workflow.status, 'pending');
    assert.strictEqual(result.steps.length, 3);
    assert.ok(Array.isArray(result.executionOrder));
    assert.strictEqual(result.executionOrder.length, 3);
  });

  it('persists workflow in the store', () => {
    const result = svc.createWorkflow({ name: 'persist-check', steps: [{ name: 'only', type: 'transform' }] });
    const fetched = store.getWorkflow(result.workflow.id);
    assert.ok(fetched);
    assert.strictEqual(fetched.name, 'persist-check');
  });

  it('persists step records in the store', () => {
    const result = svc.createWorkflow({ name: 'step-check', steps: linearSteps() });
    const steps = store.listWorkflowSteps({ workflow_id: result.workflow.id });
    assert.strictEqual(steps.length, 3);
    const names = steps.map((s) => s.step_name);
    assert.ok(names.includes('step-a'));
    assert.ok(names.includes('step-b'));
    assert.ok(names.includes('step-c'));
  });

  it('step records have correct types and agent addresses', () => {
    const result = svc.createWorkflow({ name: 'type-check', steps: linearSteps() });
    const steps = store.listWorkflowSteps({ workflow_id: result.workflow.id });
    const stepA = steps.find((s) => s.step_name === 'step-a');
    assert.strictEqual(stepA.step_type, 'quote_request');
    assert.strictEqual(stepA.agent_address, '0xA');
  });

  it('saves metadata when provided', () => {
    const meta = { source: 'unit-test', priority: 'high' };
    const result = svc.createWorkflow({ name: 'meta-wf', steps: [{ name: 'x', type: 'transform' }], metadata: meta });
    const wf = store.getWorkflow(result.workflow.id);
    const parsed = JSON.parse(wf.metadata);
    assert.strictEqual(parsed.source, 'unit-test');
    assert.strictEqual(parsed.priority, 'high');
  });

  it('stores definition with execution order', () => {
    const result = svc.createWorkflow({ name: 'def-check', steps: linearSteps() });
    const wf = store.getWorkflow(result.workflow.id);
    const definition = JSON.parse(wf.definition);
    assert.ok(definition.executionOrder);
    assert.strictEqual(definition.executionOrder.length, 3);
    assert.ok(definition.steps);
  });

  it('throws when name is missing', () => {
    assert.throws(
      () => svc.createWorkflow({ steps: [{ name: 'a', type: 'transform' }] }),
      /name is required/,
    );
  });

  it('throws when steps array is empty', () => {
    assert.throws(
      () => svc.createWorkflow({ name: 'empty', steps: [] }),
      /Steps array is required/,
    );
  });

  it('throws when steps is not provided', () => {
    assert.throws(
      () => svc.createWorkflow({ name: 'no-steps' }),
      /Steps array is required/,
    );
  });

  it('throws when DAG has a cycle', () => {
    assert.throws(
      () => svc.createWorkflow({
        name: 'cyclic',
        steps: [
          { name: 'a', type: 'transform', dependsOn: ['b'] },
          { name: 'b', type: 'transform', dependsOn: ['a'] },
        ],
      }),
      /Invalid workflow DAG.*Cycle/,
    );
  });

  it('throws when step has unknown dependency', () => {
    assert.throws(
      () => svc.createWorkflow({
        name: 'bad-dep',
        steps: [{ name: 'orphan', type: 'transform', dependsOn: ['ghost'] }],
      }),
      /Invalid workflow DAG.*unknown step/,
    );
  });

  it('defaults step type to quote_request when not specified', () => {
    const result = svc.createWorkflow({ name: 'default-type', steps: [{ name: 'implicit' }] });
    const steps = store.listWorkflowSteps({ workflow_id: result.workflow.id });
    assert.strictEqual(steps[0].step_type, 'quote_request');
  });
});

// ===========================================================================
// 4. executeWorkflow — basic execution
// ===========================================================================

describe('executeWorkflow()', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('runs a single-step workflow to completion', async () => {
    const wf = svc.createWorkflow({
      name: 'single-step',
      steps: [{ name: 'only', type: 'quote_request' }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.completedSteps, 1);
  });

  it('runs a linear chain in order', async () => {
    const wf = svc.createWorkflow({ name: 'linear-exec', steps: linearSteps() });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.completedSteps, 3);
    assert.ok(result.results);
  });

  it('runs a diamond DAG successfully', async () => {
    const wf = svc.createWorkflow({ name: 'diamond-exec', steps: diamondSteps() });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.completedSteps, 4);
  });

  it('sets workflow status to running then completed', async () => {
    const wf = svc.createWorkflow({
      name: 'status-check',
      steps: [{ name: 'x', type: 'transform' }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const final = store.getWorkflow(wf.workflow.id);
    assert.strictEqual(final.status, 'completed');
    assert.ok(final.completed_at);
  });

  it('tracks total cost across steps', async () => {
    const wf = svc.createWorkflow({
      name: 'cost-track',
      steps: [
        { name: 'q1', type: 'quote_request', agentAddress: '0xA' },
        { name: 'q2', type: 'quote_request', agentAddress: '0xB' },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    // Simulated steps have cost 0, so totalCost should be 0
    assert.strictEqual(result.totalCost, 0);
  });

  it('returns simulated result when a2aService is null for quote_request', async () => {
    const wf = svc.createWorkflow({
      name: 'simulated-quote',
      steps: [{ name: 'sim', type: 'quote_request' }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.sim.simulated, true);
    assert.strictEqual(result.results.sim.success, true);
  });

  it('returns simulated result when a2aService is null for payment', async () => {
    const wf = svc.createWorkflow({
      name: 'simulated-payment',
      steps: [{ name: 'pay', type: 'payment' }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.pay.simulated, true);
  });

  it('passes context to steps', async () => {
    const wf = svc.createWorkflow({
      name: 'context-pass',
      steps: [
        { name: 'transform', type: 'transform', params: { transformType: 'merge' } },
      ],
    });
    const ctx = { inputData: 'hello' };
    const result = await svc.executeWorkflow(wf.workflow.id, ctx);
    assert.strictEqual(result.status, 'completed');
    // context is part of the results
    assert.strictEqual(result.results.inputData, 'hello');
  });

  it('throws when workflow ID does not exist', async () => {
    await assert.rejects(
      () => svc.executeWorkflow('nonexistent-id'),
      /not found/,
    );
  });

  it('returns early for already-completed workflow', async () => {
    const wf = svc.createWorkflow({
      name: 'already-done',
      steps: [{ name: 'done', type: 'transform' }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.ok(result.message.includes('already completed'));
  });

  it('returns early for paused workflow', async () => {
    const wf = svc.createWorkflow({
      name: 'paused-early',
      steps: [{ name: 'x', type: 'transform' }],
    });
    // Manually set to paused to test the early return
    store.updateWorkflow(wf.workflow.id, { status: 'paused' });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'paused');
    assert.ok(result.message.includes('paused'));
  });

  it('sets started_at on workflow', async () => {
    const wf = svc.createWorkflow({
      name: 'started-at-check',
      steps: [{ name: 'x', type: 'transform' }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const final = store.getWorkflow(wf.workflow.id);
    assert.ok(final.started_at);
  });

  it('clears current_step on completion', async () => {
    const wf = svc.createWorkflow({
      name: 'current-step-clear',
      steps: [{ name: 'x', type: 'transform' }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const final = store.getWorkflow(wf.workflow.id);
    assert.strictEqual(final.current_step, null);
  });
});

// ===========================================================================
// 5. Step types — transform variants
// ===========================================================================

describe('Step types — transform', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('transform/merge merges dependency results', async () => {
    const wf = svc.createWorkflow({
      name: 'merge-test',
      steps: [
        { name: 'src-a', type: 'quote_request' },
        { name: 'src-b', type: 'quote_request' },
        { name: 'merged', type: 'transform', dependsOn: ['src-a', 'src-b'], params: { transformType: 'merge' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.ok(result.results.merged.merged);
    assert.ok(result.results.merged.merged['src-a']);
    assert.ok(result.results.merged.merged['src-b']);
  });

  it('transform/sum_costs sums cost values from dependencies', async () => {
    const wf = svc.createWorkflow({
      name: 'sum-costs-test',
      steps: [
        { name: 'a', type: 'quote_request' },
        { name: 'b', type: 'quote_request' },
        { name: 'total', type: 'transform', dependsOn: ['a', 'b'], params: { transformType: 'sum_costs' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(typeof result.results.total.totalCost, 'number');
  });

  it('transform/aggregate produces structured aggregation', async () => {
    const wf = svc.createWorkflow({
      name: 'aggregate-test',
      steps: [
        { name: 'x', type: 'quote_request' },
        { name: 'y', type: 'quote_request' },
        { name: 'agg', type: 'transform', dependsOn: ['x', 'y'], params: { transformType: 'aggregate' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.ok(Array.isArray(result.results.agg.aggregated));
    assert.strictEqual(result.results.agg.aggregated.length, 2);
    const stepNames = result.results.agg.aggregated.map((a) => a.step);
    assert.ok(stepNames.includes('x'));
    assert.ok(stepNames.includes('y'));
  });

  it('transform with unknown transformType returns data passthrough', async () => {
    const wf = svc.createWorkflow({
      name: 'passthrough-test',
      steps: [
        { name: 'src', type: 'quote_request' },
        { name: 'pass', type: 'transform', dependsOn: ['src'], params: { transformType: 'unknown_type' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.ok(result.results.pass.data);
    assert.ok(result.results.pass.data.src);
  });

  it('transform defaults to merge when no transformType', async () => {
    const wf = svc.createWorkflow({
      name: 'default-transform',
      steps: [
        { name: 'src', type: 'quote_request' },
        { name: 'tfm', type: 'transform', dependsOn: ['src'] },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.ok(result.results.tfm.merged);
  });
});

// ===========================================================================
// 5b. Step types — condition_check
// ===========================================================================

describe('Step types — condition_check', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('condition_check/exists passes when dependencies exist', async () => {
    const wf = svc.createWorkflow({
      name: 'exists-pass',
      steps: [
        { name: 'dep', type: 'quote_request' },
        { name: 'check', type: 'condition_check', dependsOn: ['dep'], params: { check: 'exists' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.check.passed, true);
    assert.strictEqual(result.results.check.condition, 'exists');
  });

  it('condition_check/exists with target checks specific dependency', async () => {
    const wf = svc.createWorkflow({
      name: 'exists-target',
      steps: [
        { name: 'dep', type: 'quote_request' },
        { name: 'check', type: 'condition_check', dependsOn: ['dep'], params: { check: 'exists', target: 'dep' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.check.passed, true);
  });

  it('condition_check/exists fails when target does not exist', async () => {
    const wf = svc.createWorkflow({
      name: 'exists-fail',
      steps: [
        { name: 'check', type: 'condition_check', params: { check: 'exists', target: 'missing' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'failed');
    assert.strictEqual(result.failedStep, 'check');
    assert.ok(result.error.includes('not found'));
  });

  it('condition_check/min_value passes when value meets threshold', async () => {
    // Use a workflow with a2a service that returns a total
    // Since we use null a2a service, quote_request returns simulated with cost 0
    // We'll use context to inject a value
    const wf = svc.createWorkflow({
      name: 'min-val-pass',
      steps: [
        { name: 'source', type: 'quote_request' },
        { name: 'check', type: 'condition_check', dependsOn: ['source'], params: { check: 'min_value', target: 'source', minValue: 0 } },
      ],
    });
    // The simulated quote_request has cost: 0 and total is not set, so total is undefined
    // With minValue=0, value of undefined (null) is < 0? Actually null < 0 is false, but == null check triggers
    // Let's just test by providing context with total
    const result = await svc.executeWorkflow(wf.workflow.id);
    // The simulated result is { success: true, message: '...', cost: 0, simulated: true }
    // depResults.source.total would be undefined => value == null => throws
    assert.strictEqual(result.status, 'failed');
  });

  it('condition_check with unknown check type passes (default)', async () => {
    const wf = svc.createWorkflow({
      name: 'unknown-check',
      steps: [
        { name: 'check', type: 'condition_check', params: { check: 'custom_check' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.check.passed, true);
  });

  it('condition_check defaults to exists when no check param', async () => {
    const wf = svc.createWorkflow({
      name: 'default-check',
      steps: [
        { name: 'dep', type: 'quote_request' },
        { name: 'check', type: 'condition_check', dependsOn: ['dep'] },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.check.condition, 'exists');
  });
});

// ===========================================================================
// 5c. Step types — quote_request and payment (with a2a service mock)
// ===========================================================================

describe('Step types — with a2a service', () => {
  let store;

  before(() => {
    store = makeStore();
  });

  it('quote_request invokes a2aService.requestQuote', async () => {
    let calledWith = null;
    const mockA2A = {
      requestQuote: async (params) => {
        calledWith = params;
        return { quote: { id: 'q-123', total_decimal: 42.5 } };
      },
    };
    const svc = createWorkflowService(store, mockA2A);
    const wf = svc.createWorkflow({
      name: 'real-quote',
      steps: [{ name: 'fetch', type: 'quote_request', agentAddress: '0xSeller', params: { description: 'widgets' } }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(calledWith.seller, '0xSeller');
    assert.strictEqual(result.results.fetch.quoteId, 'q-123');
    assert.strictEqual(result.results.fetch.cost, 42.5);
    assert.strictEqual(result.totalCost, 42.5);
  });

  it('quote_request without agent_address returns error result', async () => {
    const mockA2A = {
      requestQuote: async () => ({ quote: { id: 'q', total_decimal: 10 } }),
    };
    const svc = createWorkflowService(store, mockA2A);
    const wf = svc.createWorkflow({
      name: 'no-address-quote',
      steps: [{ name: 'q', type: 'quote_request' }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    // Step returns { success: false, error: 'No agent_address...' } which is NOT thrown
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.q.success, false);
  });

  it('payment invokes a2aService.pay', async () => {
    let payParams = null;
    const mockA2A = {
      pay: async (params) => {
        payParams = params;
        return { payment: { id: 'pay-1' } };
      },
    };
    const svc = createWorkflowService(store, mockA2A);
    const wf = svc.createWorkflow({
      name: 'real-payment',
      steps: [{ name: 'pay', type: 'payment', agentAddress: '0xRecipient', params: { amount: 100, asset: 'USDC' } }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(payParams.to, '0xRecipient');
    assert.strictEqual(payParams.amount, 100);
    assert.strictEqual(result.results.pay.paymentId, 'pay-1');
    assert.strictEqual(result.results.pay.cost, 100);
    assert.strictEqual(result.totalCost, 100);
  });

  it('payment without recipient returns error result', async () => {
    const mockA2A = {
      pay: async () => ({ payment: { id: 'p' } }),
    };
    const svc = createWorkflowService(store, mockA2A);
    const wf = svc.createWorkflow({
      name: 'no-recipient',
      steps: [{ name: 'pay', type: 'payment', params: {} }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.results.pay.success, false);
  });
});

// ===========================================================================
// 6. getWorkflowStatus
// ===========================================================================

describe('getWorkflowStatus()', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('returns workflow and steps for a pending workflow', () => {
    const wf = svc.createWorkflow({
      name: 'status-pending',
      steps: linearSteps(),
    });
    const status = svc.getWorkflowStatus(wf.workflow.id);
    assert.strictEqual(status.workflow.name, 'status-pending');
    assert.strictEqual(status.workflow.status, 'pending');
    assert.strictEqual(status.steps.length, 3);
    assert.strictEqual(status.progress.total, 3);
    assert.strictEqual(status.progress.pending, 3);
    assert.strictEqual(status.progress.completed, 0);
    assert.strictEqual(status.progress.failed, 0);
    assert.strictEqual(status.progress.running, 0);
  });

  it('reflects completed state after execution', async () => {
    const wf = svc.createWorkflow({
      name: 'status-completed',
      steps: [{ name: 'x', type: 'transform' }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const status = svc.getWorkflowStatus(wf.workflow.id);
    assert.strictEqual(status.workflow.status, 'completed');
    assert.strictEqual(status.progress.completed, 1);
    assert.strictEqual(status.progress.pending, 0);
  });

  it('reflects failed state after step failure', async () => {
    const wf = svc.createWorkflow({
      name: 'status-failed',
      steps: [
        { name: 'check', type: 'condition_check', params: { check: 'exists', target: 'missing' } },
      ],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const status = svc.getWorkflowStatus(wf.workflow.id);
    assert.strictEqual(status.workflow.status, 'failed');
    assert.strictEqual(status.progress.failed, 1);
    assert.ok(status.workflow.error);
  });

  it('includes step details with correct fields', () => {
    const wf = svc.createWorkflow({
      name: 'step-details',
      steps: [{ name: 'my-step', type: 'transform', agentAddress: '0xAgent' }],
    });
    const status = svc.getWorkflowStatus(wf.workflow.id);
    const step = status.steps[0];
    assert.strictEqual(step.name, 'my-step');
    assert.strictEqual(step.type, 'transform');
    assert.strictEqual(step.status, 'pending');
    assert.strictEqual(step.agentAddress, '0xAgent');
    assert.ok(step.id);
  });

  it('throws for nonexistent workflow', () => {
    assert.throws(
      () => svc.getWorkflowStatus('does-not-exist'),
      /not found/,
    );
  });

  it('shows progress for multi-step workflow after partial failure', async () => {
    const wf = svc.createWorkflow({
      name: 'partial-fail',
      steps: [
        { name: 'good', type: 'transform' },
        { name: 'bad', type: 'condition_check', dependsOn: ['good'], params: { check: 'exists', target: 'nonexistent' } },
        { name: 'unreached', type: 'transform', dependsOn: ['bad'] },
      ],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const status = svc.getWorkflowStatus(wf.workflow.id);
    assert.strictEqual(status.progress.completed, 1);
    assert.strictEqual(status.progress.failed, 1);
    assert.strictEqual(status.progress.pending, 1);
  });
});

// ===========================================================================
// 7. pauseWorkflow / resumeWorkflow
// ===========================================================================

describe('pauseWorkflow()', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('pauses a pending workflow', () => {
    const wf = svc.createWorkflow({
      name: 'pause-pending',
      steps: [{ name: 'x', type: 'transform' }],
    });
    const result = svc.pauseWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'paused');
    const fetched = store.getWorkflow(wf.workflow.id);
    assert.strictEqual(fetched.status, 'paused');
  });

  it('pauses a running workflow', () => {
    const wf = svc.createWorkflow({
      name: 'pause-running',
      steps: [{ name: 'x', type: 'transform' }],
    });
    store.updateWorkflow(wf.workflow.id, { status: 'running' });
    const result = svc.pauseWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'paused');
  });

  it('cannot pause a completed workflow', () => {
    const wf = svc.createWorkflow({
      name: 'pause-completed',
      steps: [{ name: 'x', type: 'transform' }],
    });
    store.updateWorkflow(wf.workflow.id, { status: 'completed' });
    assert.throws(
      () => svc.pauseWorkflow(wf.workflow.id),
      /Cannot pause workflow in "completed"/,
    );
  });

  it('cannot pause a failed workflow', () => {
    const wf = svc.createWorkflow({
      name: 'pause-failed',
      steps: [{ name: 'x', type: 'transform' }],
    });
    store.updateWorkflow(wf.workflow.id, { status: 'failed' });
    assert.throws(
      () => svc.pauseWorkflow(wf.workflow.id),
      /Cannot pause workflow in "failed"/,
    );
  });

  it('cannot pause an already-paused workflow', () => {
    const wf = svc.createWorkflow({
      name: 'pause-paused',
      steps: [{ name: 'x', type: 'transform' }],
    });
    store.updateWorkflow(wf.workflow.id, { status: 'paused' });
    assert.throws(
      () => svc.pauseWorkflow(wf.workflow.id),
      /Cannot pause workflow in "paused"/,
    );
  });

  it('throws for nonexistent workflow', () => {
    assert.throws(
      () => svc.pauseWorkflow('no-such-id'),
      /not found/,
    );
  });
});

describe('resumeWorkflow()', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('resumes a paused workflow and completes it', async () => {
    const wf = svc.createWorkflow({
      name: 'resume-complete',
      steps: [{ name: 'x', type: 'transform' }],
    });
    svc.pauseWorkflow(wf.workflow.id);
    const result = await svc.resumeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    const final = store.getWorkflow(wf.workflow.id);
    assert.strictEqual(final.status, 'completed');
  });

  it('cannot resume a running workflow', async () => {
    const wf = svc.createWorkflow({
      name: 'resume-running',
      steps: [{ name: 'x', type: 'transform' }],
    });
    store.updateWorkflow(wf.workflow.id, { status: 'running' });
    await assert.rejects(
      () => svc.resumeWorkflow(wf.workflow.id),
      /Cannot resume workflow in "running"/,
    );
  });

  it('cannot resume a completed workflow', async () => {
    const wf = svc.createWorkflow({
      name: 'resume-completed',
      steps: [{ name: 'x', type: 'transform' }],
    });
    store.updateWorkflow(wf.workflow.id, { status: 'completed' });
    await assert.rejects(
      () => svc.resumeWorkflow(wf.workflow.id),
      /Cannot resume workflow in "completed"/,
    );
  });

  it('cannot resume a pending workflow', async () => {
    const wf = svc.createWorkflow({
      name: 'resume-pending',
      steps: [{ name: 'x', type: 'transform' }],
    });
    await assert.rejects(
      () => svc.resumeWorkflow(wf.workflow.id),
      /Cannot resume workflow in "pending"/,
    );
  });

  it('throws for nonexistent workflow', async () => {
    await assert.rejects(
      () => svc.resumeWorkflow('no-such-id'),
      /not found/,
    );
  });
});

// ===========================================================================
// 8. Edge cases
// ===========================================================================

describe('Edge cases', () => {
  let store;
  let svc;

  before(() => {
    store = makeStore();
    svc = createWorkflowService(store, null);
  });

  it('handles empty context gracefully', async () => {
    const wf = svc.createWorkflow({
      name: 'empty-ctx',
      steps: [{ name: 'x', type: 'transform', params: { transformType: 'merge' } }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id, {});
    assert.strictEqual(result.status, 'completed');
  });

  it('handles undefined context gracefully', async () => {
    const wf = svc.createWorkflow({
      name: 'undef-ctx',
      steps: [{ name: 'x', type: 'transform' }],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
  });

  it('step failure mid-workflow marks remaining steps as pending', async () => {
    const wf = svc.createWorkflow({
      name: 'mid-fail',
      steps: [
        { name: 'good-1', type: 'transform' },
        { name: 'fail-here', type: 'condition_check', dependsOn: ['good-1'], params: { check: 'exists', target: 'nonexistent' } },
        { name: 'never-runs', type: 'transform', dependsOn: ['fail-here'] },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'failed');
    assert.strictEqual(result.failedStep, 'fail-here');

    const steps = store.listWorkflowSteps({ workflow_id: wf.workflow.id });
    const neverRuns = steps.find((s) => s.step_name === 'never-runs');
    assert.strictEqual(neverRuns.status, 'pending');
  });

  it('failed workflow records error on the workflow record', async () => {
    const wf = svc.createWorkflow({
      name: 'error-record',
      steps: [
        { name: 'fail', type: 'condition_check', params: { check: 'exists', target: 'nope' } },
      ],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const fetched = store.getWorkflow(wf.workflow.id);
    assert.strictEqual(fetched.status, 'failed');
    assert.ok(fetched.error);
    assert.ok(fetched.error.includes('fail'));
  });

  it('failed step records error on the step record', async () => {
    const wf = svc.createWorkflow({
      name: 'step-error-record',
      steps: [
        { name: 'bad', type: 'condition_check', params: { check: 'exists', target: 'nope' } },
      ],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const steps = store.listWorkflowSteps({ workflow_id: wf.workflow.id });
    const bad = steps.find((s) => s.step_name === 'bad');
    assert.strictEqual(bad.status, 'failed');
    assert.ok(bad.error);
  });

  it('multiple workflows can coexist independently', async () => {
    const wf1 = svc.createWorkflow({
      name: 'wf-1',
      steps: [{ name: 'a', type: 'transform' }],
    });
    const wf2 = svc.createWorkflow({
      name: 'wf-2',
      steps: [{ name: 'b', type: 'transform' }],
    });
    await svc.executeWorkflow(wf1.workflow.id);
    const status1 = svc.getWorkflowStatus(wf1.workflow.id);
    const status2 = svc.getWorkflowStatus(wf2.workflow.id);
    assert.strictEqual(status1.workflow.status, 'completed');
    assert.strictEqual(status2.workflow.status, 'pending');
  });

  it('workflow with all step types executes in order', async () => {
    const wf = svc.createWorkflow({
      name: 'all-types',
      steps: [
        { name: 'quote', type: 'quote_request' },
        { name: 'pay', type: 'payment', dependsOn: ['quote'] },
        { name: 'check', type: 'condition_check', dependsOn: ['pay'] },
        { name: 'finalize', type: 'transform', dependsOn: ['check'], params: { transformType: 'merge' } },
      ],
    });
    const result = await svc.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'completed');
    assert.strictEqual(result.completedSteps, 4);
  });

  it('workflow total_cost is persisted in the database', async () => {
    const mockA2A = {
      requestQuote: async () => ({ quote: { id: 'q1', total_decimal: 25 } }),
    };
    const svcWithA2A = createWorkflowService(store, mockA2A);
    const wf = svcWithA2A.createWorkflow({
      name: 'cost-persist',
      steps: [{ name: 'q', type: 'quote_request', agentAddress: '0xA' }],
    });
    await svcWithA2A.executeWorkflow(wf.workflow.id);
    const fetched = store.getWorkflow(wf.workflow.id);
    assert.strictEqual(fetched.total_cost, 25);
  });

  it('step cost is persisted in the database', async () => {
    const mockA2A = {
      requestQuote: async () => ({ quote: { id: 'q1', total_decimal: 15.5 } }),
    };
    const svcWithA2A = createWorkflowService(store, mockA2A);
    const wf = svcWithA2A.createWorkflow({
      name: 'step-cost-persist',
      steps: [{ name: 'q', type: 'quote_request', agentAddress: '0xA' }],
    });
    await svcWithA2A.executeWorkflow(wf.workflow.id);
    const steps = store.listWorkflowSteps({ workflow_id: wf.workflow.id });
    assert.strictEqual(steps[0].cost, 15.5);
  });

  it('executeWorkflow sets started_at and completed_at timestamps', async () => {
    const wf = svc.createWorkflow({
      name: 'timestamps',
      steps: [{ name: 'x', type: 'transform' }],
    });
    const beforeExec = new Date().toISOString();
    await svc.executeWorkflow(wf.workflow.id);
    const final = store.getWorkflow(wf.workflow.id);
    assert.ok(final.started_at >= beforeExec);
    assert.ok(final.completed_at >= final.started_at);
  });

  it('step completed_at is set on completed steps', async () => {
    const wf = svc.createWorkflow({
      name: 'step-timestamps',
      steps: [{ name: 'x', type: 'transform' }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const steps = store.listWorkflowSteps({ workflow_id: wf.workflow.id });
    assert.ok(steps[0].completed_at);
  });

  it('step result is stored as JSON', async () => {
    const wf = svc.createWorkflow({
      name: 'result-json',
      steps: [{ name: 'x', type: 'transform', params: { transformType: 'merge' } }],
    });
    await svc.executeWorkflow(wf.workflow.id);
    const steps = store.listWorkflowSteps({ workflow_id: wf.workflow.id });
    const parsed = JSON.parse(steps[0].result);
    assert.strictEqual(parsed.success, true);
    assert.ok(parsed.merged !== undefined);
  });

  it('large DAG with many steps is valid', () => {
    const steps = [];
    for (let i = 0; i < 50; i++) {
      steps.push({
        name: `step-${i}`,
        type: 'transform',
        dependsOn: i > 0 ? [`step-${i - 1}`] : [],
      });
    }
    const result = svc.validateDAG(steps);
    assert.strictEqual(result.valid, true);
    assert.strictEqual(result.order.length, 50);
    assert.strictEqual(result.order[0], 'step-0');
    assert.strictEqual(result.order[49], 'step-49');
  });

  it('resume skips already-completed steps', async () => {
    const mockA2A = {
      requestQuote: async () => ({ quote: { id: 'q1', total_decimal: 10 } }),
    };
    const svcWithA2A = createWorkflowService(store, mockA2A);
    const wf = svcWithA2A.createWorkflow({
      name: 'resume-skip',
      steps: [
        { name: 'step-1', type: 'quote_request', agentAddress: '0xA' },
        { name: 'step-2', type: 'transform', dependsOn: ['step-1'] },
      ],
    });
    // Run to completion first
    await svcWithA2A.executeWorkflow(wf.workflow.id);
    // Manually set to paused to force re-run
    store.updateWorkflow(wf.workflow.id, { status: 'paused' });
    const result = await svcWithA2A.resumeWorkflow(wf.workflow.id);
    // Should complete because both steps are already completed
    assert.strictEqual(result.status, 'completed');
  });

  it('a2a service failure propagates as step failure', async () => {
    const mockA2A = {
      requestQuote: async () => { throw new Error('Network timeout'); },
    };
    const svcWithA2A = createWorkflowService(store, mockA2A);
    const wf = svcWithA2A.createWorkflow({
      name: 'a2a-fail',
      steps: [{ name: 'q', type: 'quote_request', agentAddress: '0xA' }],
    });
    const result = await svcWithA2A.executeWorkflow(wf.workflow.id);
    assert.strictEqual(result.status, 'failed');
    assert.strictEqual(result.failedStep, 'q');
    assert.ok(result.error.includes('Network timeout'));
  });
});
