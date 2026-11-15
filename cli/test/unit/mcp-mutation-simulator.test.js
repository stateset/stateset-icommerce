// Unit tests for the capability-scope paths of cli/src/mcp/mutation-simulator.js
//
// A tool a toolkit capability scope excludes reads as unknown to the server
// but carries an `unavailable` refusal in its runtime metadata. Simulation
// and replay must refuse it by name and never hand it to the step executor.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  createReplayMutationToolCall,
  createSimulateMutationToolCall,
} from '../../src/mcp/mutation-simulator.js';

const MESSAGE = (name) => `Tool '${name}' is outside this toolkit's capability scope.`;
const EXPOSED = new Map([
  ['create_customer', 'write'],
  ['list_customers', 'read'],
]);
const EXCLUDED = new Map([
  ['delete_customer', 'write'],
  ['export_full_data', 'read'],
]);

function getToolRuntimeMeta(name) {
  if (EXPOSED.has(name)) {
    const permission = EXPOSED.get(name);
    return {
      name,
      permission,
      policyDomain: 'customers',
      sideEffect: permission === 'read' ? 'read' : 'write',
      compensations: [],
      idempotent: false,
    };
  }
  return {
    name,
    permission: 'unknown',
    policyDomain: 'customers',
    sideEffect: 'unknown',
    compensations: [],
    idempotent: false,
    ...(EXCLUDED.has(name)
      ? {
          unavailable: {
            reason: 'outside_capability_scope',
            message: MESSAGE(name),
            sideEffect: EXCLUDED.get(name) === 'read' ? 'read' : 'write',
          },
        }
      : {}),
  };
}

function makeDeps(events = []) {
  const executed = [];
  const logged = [];
  return {
    executed,
    logged,
    deps: {
      getToolRuntimeMeta,
      inferPolicyDomain: () => 'customers',
      executeToolStepInPlan: async (input) => {
        executed.push(input);
        return { index: 0, tool: input.toolName, status: 'dry_run_success', paramsHash: 'h' };
      },
      addAgenticReplayEvent: async (event) => {
        logged.push(event);
      },
      listAgenticReplayEvents: async () => ({ events }),
    },
  };
}

describe('simulateMutationToolCall capability scope', () => {
  it('refuses an excluded tool by name and runs nothing', async () => {
    const { deps, executed, logged } = makeDeps();
    const simulate = createSimulateMutationToolCall(deps);
    const out = await simulate({ tool: 'delete_customer', params: { customerId: 'c1' } });
    assert.deepEqual(out, {
      success: false,
      error: MESSAGE('delete_customer'),
      errorCode: 'PERMISSION_DENIED',
      refusal: 'outside_capability_scope',
    });
    assert.equal(executed.length, 0);
    assert.equal(logged.length, 0);
  });

  it('keeps reporting tools the server does not know as unknown', async () => {
    const { deps } = makeDeps();
    const out = await createSimulateMutationToolCall(deps)({ tool: 'nope' });
    assert.deepEqual(out, { success: false, error: "Unknown tool 'nope'" });
  });
});

describe('replayMutationToolCall capability scope', () => {
  const event = (tool, extra = {}) => ({
    eventId: `evt-${tool}`,
    requestId: 'req-1',
    tool,
    params: { customerId: 'c1' },
    status: 'preview',
    ...extra,
  });

  it('refuses the newest matching mutation when its tool is excluded', async () => {
    const { deps, executed, logged } = makeDeps([
      event('export_full_data'),
      event('delete_customer'),
      event('create_customer'),
    ]);
    const out = await createReplayMutationToolCall(deps)({ requestId: 'req-1', dryRun: false });
    // export_full_data is a read tool: skipped as before. delete_customer is
    // the first replayable mutation and is refused, not silently passed over.
    assert.equal(out.success, false);
    assert.equal(out.error, MESSAGE('delete_customer'));
    assert.equal(out.errorCode, 'PERMISSION_DENIED');
    assert.equal(out.refusal, 'outside_capability_scope');
    assert.equal(out.sourceEvent.eventId, 'evt-delete_customer');
    assert.equal(executed.length, 0);
    assert.equal(logged.length, 0);
  });

  it('replays an exposed mutation exactly as before', async () => {
    const { deps, executed } = makeDeps([event('nope'), event('create_customer')]);
    const out = await createReplayMutationToolCall(deps)({ requestId: 'req-1' });
    assert.equal(out.success, true);
    assert.equal(out.sourceEvent.tool, 'create_customer');
    assert.equal(executed.length, 1);
    assert.equal(executed[0].dryRun, true);
  });
});
