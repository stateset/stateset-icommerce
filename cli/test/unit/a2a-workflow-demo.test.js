/**
 * Tests for the workflow-pipeline demo scenario.
 *
 * Validates runWorkflowPipeline(), DEMO_SCENARIOS, and runDemoScenario()
 * routing for the 3-agent DAG pipeline (DataFetcher -> Analyzer -> ReportGenerator).
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { A2AStore } from '../../src/a2a/store.js';
import { makeCommerceProxy } from '../../src/a2a/agent-runtime.js';
import {
  runWorkflowPipeline,
  runDemoScenario,
  DEMO_SCENARIOS,
} from '../../src/a2a/demo-scenarios.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function setup() {
  const store = new A2AStore({ dbPath: ':memory:' });
  store.init();
  const commerce = makeCommerceProxy(store);
  return { store, commerce };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('a2a-workflow-demo — DEMO_SCENARIOS registry', () => {
  it('DEMO_SCENARIOS includes workflow-pipeline', () => {
    assert.ok(DEMO_SCENARIOS.includes('workflow-pipeline'));
  });

  it('DEMO_SCENARIOS is a non-empty array', () => {
    assert.ok(Array.isArray(DEMO_SCENARIOS));
    assert.ok(DEMO_SCENARIOS.length > 0);
  });

  it('DEMO_SCENARIOS contains exactly the expected 5 scenarios', () => {
    assert.strictEqual(DEMO_SCENARIOS.length, 5);
    const expected = [
      'basic-negotiation',
      'marketplace',
      'escrow-deal',
      'rfq-competition',
      'workflow-pipeline',
    ];
    for (const name of expected) {
      assert.ok(DEMO_SCENARIOS.includes(name), `Missing scenario: ${name}`);
    }
  });
});

describe('a2a-workflow-demo — runDemoScenario routing', () => {
  let store, commerce;

  beforeEach(() => {
    ({ store, commerce } = setup());
  });

  afterEach(() => {
    store.close();
  });

  it('runDemoScenario("workflow-pipeline") routes to runWorkflowPipeline', async () => {
    const result = await runDemoScenario('workflow-pipeline', commerce, { log: () => {} });
    assert.strictEqual(result.scenario, 'workflow-pipeline');
  });

  it('runDemoScenario throws for unknown scenario names', async () => {
    await assert.rejects(
      () => runDemoScenario('nonexistent-scenario', commerce, { log: () => {} }),
      (err) => {
        assert.ok(err.message.includes('Unknown demo scenario'));
        assert.ok(err.message.includes('nonexistent-scenario'));
        return true;
      },
    );
  });

  it('runDemoScenario("workflow-pipeline") returns same shape as direct call', async () => {
    const routed = await runDemoScenario('workflow-pipeline', commerce, { log: () => {} });
    assert.strictEqual(routed.scenario, 'workflow-pipeline');
    assert.ok('workflowId' in routed);
    assert.ok('status' in routed);
    assert.ok('totalCost' in routed);
    assert.ok('completedSteps' in routed);
    assert.ok('stepDetails' in routed);
  });
});

describe('a2a-workflow-demo — runWorkflowPipeline return shape', () => {
  let store, commerce, result;

  beforeEach(async () => {
    ({ store, commerce } = setup());
    result = await runWorkflowPipeline(commerce, { log: () => {} });
  });

  afterEach(() => {
    store.close();
  });

  it('returns scenario = "workflow-pipeline"', () => {
    assert.strictEqual(result.scenario, 'workflow-pipeline');
  });

  it('returns a workflowId string', () => {
    assert.strictEqual(typeof result.workflowId, 'string');
    assert.ok(result.workflowId.length > 0);
  });

  it('returns a status string', () => {
    assert.strictEqual(typeof result.status, 'string');
    assert.ok(
      ['completed', 'failed'].includes(result.status),
      `Unexpected status: ${result.status}`,
    );
  });

  it('returns totalCost as a number', () => {
    assert.strictEqual(typeof result.totalCost, 'number');
    assert.ok(result.totalCost >= 0, 'totalCost should be non-negative');
  });

  it('returns completedSteps as a number', () => {
    assert.strictEqual(typeof result.completedSteps, 'number');
    assert.ok(result.completedSteps >= 0);
  });

  it('returns stepDetails as an array', () => {
    assert.ok(Array.isArray(result.stepDetails));
  });
});

describe('a2a-workflow-demo — step completion', () => {
  let store, commerce, result;

  beforeEach(async () => {
    ({ store, commerce } = setup());
    result = await runWorkflowPipeline(commerce, { log: () => {} });
  });

  afterEach(() => {
    store.close();
  });

  it('all 3 steps are present in stepDetails', () => {
    assert.strictEqual(result.stepDetails.length, 3);
  });

  it('step names are fetch, analyze, report', () => {
    const names = result.stepDetails.map((s) => s.name);
    assert.deepStrictEqual(names, ['fetch', 'analyze', 'report']);
  });

  it('steps are in dependency order (fetch before analyze before report)', () => {
    const names = result.stepDetails.map((s) => s.name);
    assert.ok(names.indexOf('fetch') < names.indexOf('analyze'));
    assert.ok(names.indexOf('analyze') < names.indexOf('report'));
  });

  it('completedSteps equals 3 when workflow completes successfully', () => {
    if (result.status === 'completed') {
      assert.strictEqual(result.completedSteps, 3);
    }
  });
});

describe('a2a-workflow-demo — step details structure', () => {
  let store, commerce, result;

  beforeEach(async () => {
    ({ store, commerce } = setup());
    result = await runWorkflowPipeline(commerce, { log: () => {} });
  });

  afterEach(() => {
    store.close();
  });

  it('each step has name, type, status, and cost properties', () => {
    for (const step of result.stepDetails) {
      assert.ok('name' in step, `Step missing "name": ${JSON.stringify(step)}`);
      assert.ok('type' in step, `Step missing "type": ${JSON.stringify(step)}`);
      assert.ok('status' in step, `Step missing "status": ${JSON.stringify(step)}`);
      assert.ok('cost' in step, `Step missing "cost": ${JSON.stringify(step)}`);
    }
  });

  it('step types match the workflow definition', () => {
    const typeMap = { fetch: 'quote_request', analyze: 'quote_request', report: 'transform' };
    for (const step of result.stepDetails) {
      assert.strictEqual(step.type, typeMap[step.name], `Wrong type for step "${step.name}"`);
    }
  });

  it('step cost is a number for each step', () => {
    for (const step of result.stepDetails) {
      assert.strictEqual(typeof step.cost, 'number', `Cost not a number for step "${step.name}"`);
    }
  });

  it('step status is a recognized value', () => {
    const validStatuses = new Set(['pending', 'running', 'completed', 'failed']);
    for (const step of result.stepDetails) {
      assert.ok(
        validStatuses.has(step.status),
        `Unexpected status "${step.status}" for step "${step.name}"`,
      );
    }
  });

  it('totalCost equals sum of individual step costs', () => {
    const sumOfSteps = result.stepDetails.reduce((sum, s) => sum + s.cost, 0);
    assert.strictEqual(result.totalCost, sumOfSteps);
  });
});

describe('a2a-workflow-demo — custom log function', () => {
  let store, commerce;

  beforeEach(() => {
    ({ store, commerce } = setup());
  });

  afterEach(() => {
    store.close();
  });

  it('calls the custom log function during execution', async () => {
    const logs = [];
    await runWorkflowPipeline(commerce, { log: (msg) => logs.push(msg) });
    assert.ok(logs.length > 0, 'Expected at least one log message');
  });

  it('log messages include "[demo]" prefix', async () => {
    const logs = [];
    await runWorkflowPipeline(commerce, { log: (msg) => logs.push(msg) });
    const demoLogs = logs.filter((m) => typeof m === 'string' && m.includes('[demo]'));
    assert.ok(demoLogs.length > 0, 'Expected log messages with [demo] prefix');
  });

  it('log messages reference workflow pipeline', async () => {
    const logs = [];
    await runWorkflowPipeline(commerce, { log: (msg) => logs.push(msg) });
    const pipelineLogs = logs.filter(
      (m) => typeof m === 'string' && m.toLowerCase().includes('pipeline'),
    );
    assert.ok(pipelineLogs.length > 0, 'Expected log messages referencing pipeline');
  });
});

describe('a2a-workflow-demo — edge cases', () => {
  it('works with default options (no options object)', async () => {
    const { store, commerce } = setup();
    // runWorkflowPipeline defaults log to console.log — just ensure it does not throw
    const origLog = console.log;
    console.log = () => {};
    try {
      const result = await runWorkflowPipeline(commerce);
      assert.strictEqual(result.scenario, 'workflow-pipeline');
    } finally {
      console.log = origLog;
      store.close();
    }
  });

  it('each invocation creates a unique workflowId', async () => {
    const { store, commerce } = setup();
    const noop = { log: () => {} };
    const r1 = await runWorkflowPipeline(commerce, noop);
    const r2 = await runWorkflowPipeline(commerce, noop);
    assert.notStrictEqual(r1.workflowId, r2.workflowId);
    store.close();
  });

  it('workflow data persists in the store after execution', async () => {
    const { store, commerce } = setup();
    const result = await runWorkflowPipeline(commerce, { log: () => {} });
    // Verify we can still retrieve the workflow from the store
    const wf = store.getWorkflow(result.workflowId);
    assert.ok(wf, 'Workflow should still exist in the store');
    assert.strictEqual(wf.name, 'data-pipeline');
    store.close();
  });

  it('workflow definition stored in the DB includes executionOrder', async () => {
    const { store, commerce } = setup();
    const result = await runWorkflowPipeline(commerce, { log: () => {} });
    const wf = store.getWorkflow(result.workflowId);
    const definition = JSON.parse(wf.definition);
    assert.ok(Array.isArray(definition.executionOrder));
    assert.deepStrictEqual(definition.executionOrder, ['fetch', 'analyze', 'report']);
    store.close();
  });
});
