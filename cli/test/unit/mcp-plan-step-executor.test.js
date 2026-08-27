// Unit tests for cli/src/mcp/plan-step-executor.js
//
// Covers `createExecuteToolStepInPlan` — every exit of the single-step
// gauntlet:
//  - unknown tool / tool without handler / schema validation failure
//  - before_tool_call hook rewrite + block
//  - policy block, permission preview / permission_block / dry_run_blocked
//  - MPP payment_required, treasury_block (+ dry-run variant)
//  - dry_run_success, success (with after_tool_call hook + payment receipt),
//    handler `error` result, thrown error, and rollback status mapping
//  - the shared tool context is read lazily via getToolContext()

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { createExecuteToolStepInPlan } from '../../src/mcp/plan-step-executor.js';
import { MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE } from '../../src/mpp/index.js';

function makeExecutor(overrides = {}) {
  const hookCalls = [];
  const handlerCalls = [];
  const toolDefsByName = new Map([
    [
      'create_order',
      {
        name: 'create_order',
        description: 'Create order',
        inputSchema: { customerId: z.string().min(1) },
        permission: 'write',
        handler: async (payload) => {
          handlerCalls.push(payload);
          return overrides.handlerResult ?? { id: 'ord_1', customerId: payload.params.customerId };
        },
      },
    ],
    ['no_handler', { name: 'no_handler', inputSchema: {}, permission: 'write' }],
  ]);
  const hookRunner = {
    hasHooks: () => true,
    run: async (hook, payload) => {
      hookCalls.push({ hook, payload });
      return overrides.hookResult?.[hook] ?? {};
    },
  };
  const execute = createExecuteToolStepInPlan({
    toolDefsByName,
    inferPolicyDomain: () => 'orders',
    getToolRuntimeMeta: (name) => ({
      name,
      permission: toolDefsByName.has(name) ? 'write' : 'unknown',
      policyDomain: 'orders',
      sideEffect: 'write',
      compensations: ['cancel_order'],
      idempotent: false,
    }),
    hookRunner,
    allowApply: true,
    evaluatePolicy: async (tool, params, extra, domain) => ({
      allowed: true,
      params,
      domain,
      actions: [],
    }),
    checkPermission: async () => ({ allowed: true }),
    resolveMppPaymentContext: async () => ({ pricing: null, authorized: false }),
    maybeChargeForTool: async () => ({ charged: false, blocked: false, rule: null }),
    wrapWithTelemetry: (name, fn) => fn,
    getToolContext: () => ({ commerce: 'ctx-commerce', allowApply: true }),
    ...overrides.deps,
  });
  return { execute, hookCalls, handlerCalls };
}

const base = { toolName: 'create_order', params: { customerId: 'c1' }, stepIndex: 3 };

describe('createExecuteToolStepInPlan — validation exits', () => {
  it('returns invalid for unknown tools and tools without handlers', async () => {
    const { execute } = makeExecutor();
    const unknown = await execute({ ...base, toolName: 'nope' });
    assert.equal(unknown.status, 'invalid');
    assert.equal(unknown.index, 3);
    assert.equal(unknown.error, "Unknown tool 'nope'");

    const noHandler = await execute({ ...base, toolName: 'no_handler' });
    assert.equal(noHandler.status, 'invalid');
    assert.equal(noHandler.error, "No executable handler for tool 'no_handler'");
  });

  it('returns invalid with validation notes when params fail the schema', async () => {
    const { execute } = makeExecutor();
    const out = await execute({ ...base, params: {}, dryRun: true });
    assert.equal(out.status, 'invalid');
    assert.equal(out.simulation, true);
    assert.equal(out.notes.validation[0].path, 'customerId');
    assert.equal(out.runtime.policyDomain, 'orders');
    assert.match(out.paramsHash, /^[0-9a-f]{64}$/);
  });
});

describe('createExecuteToolStepInPlan — gates', () => {
  it('lets before_tool_call rewrite params and can block', async () => {
    const { execute } = makeExecutor({
      hookResult: {
        before_tool_call: {
          blocked: true,
          reason: 'hook says no',
          params: { customerId: 'rewritten' },
        },
      },
    });
    const out = await execute(base);
    assert.equal(out.status, 'blocked');
    assert.equal(out.error, 'hook says no');
    assert.deepEqual(out.params, { customerId: 'rewritten' });
    assert.deepEqual(out.notes.hook, { allowed: undefined, reason: 'hook says no', blocked: true });
    assert.equal(out.mutationManifest.phase, 'blocked');
  });

  it('skips hooks when includeHooks is false', async () => {
    const { execute, hookCalls } = makeExecutor();
    await execute({ ...base, includeHooks: false });
    assert.equal(hookCalls.length, 0);
  });

  it('maps a policy denial to policy_block', async () => {
    const { execute } = makeExecutor({
      deps: {
        evaluatePolicy: async () => ({ allowed: false, domain: 'orders', reason: 'policy nope' }),
      },
    });
    const out = await execute(base);
    assert.equal(out.status, 'policy_block');
    assert.equal(out.error, 'policy nope');
    assert.equal(out.policy.reason, 'policy nope');
    assert.equal(out.permission, null);
  });

  it('maps permission outcomes to preview / permission_block / dry_run_blocked', async () => {
    const preview = makeExecutor({
      deps: {
        checkPermission: async () => ({
          allowed: false,
          preview: true,
          reason: 'p',
          wouldDo: { x: 1 },
        }),
      },
    });
    const a = await preview.execute(base);
    assert.equal(a.status, 'preview');
    assert.deepEqual(a.wouldDo, { x: 1 });
    const b = await preview.execute({ ...base, dryRun: true });
    assert.equal(b.status, 'dry_run_blocked');

    const denied = makeExecutor({
      deps: { checkPermission: async () => ({ allowed: false, preview: false, reason: 'denied' }) },
    });
    const c = await denied.execute(base);
    assert.equal(c.status, 'permission_block');
    assert.equal(c.error, 'denied');
  });

  it('returns payment_required when the tool is priced but unauthorized', async () => {
    const { execute } = makeExecutor({
      deps: {
        resolveMppPaymentContext: async () => ({
          pricing: { amount: '1' },
          authorized: false,
          challenge: { challengeId: 'ch_1' },
          errorPayload: { paymentRequired: true },
          verification: { reason: 'bad credential' },
        }),
      },
    });
    const out = await execute(base);
    assert.equal(out.status, 'payment_required');
    assert.equal(out.error, 'bad credential');
    assert.equal(out.charge.reason, MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE);
    assert.deepEqual(out.result, { paymentRequired: true });
  });

  it('returns treasury_block (or dry_run_blocked) when the charge is blocked', async () => {
    const { execute } = makeExecutor({
      deps: {
        maybeChargeForTool: async () => ({ charged: false, blocked: true, reason: 'no funds' }),
      },
    });
    const live = await execute(base);
    assert.equal(live.status, 'treasury_block');
    assert.equal(live.error, 'no funds');
    const dry = await execute({ ...base, dryRun: true });
    assert.equal(dry.status, 'dry_run_blocked');
  });
});

describe('createExecuteToolStepInPlan — execution', () => {
  it('returns dry_run_success without invoking the handler', async () => {
    const { execute, handlerCalls } = makeExecutor();
    const out = await execute({ ...base, dryRun: true, requestId: 'req-1' });
    assert.equal(out.status, 'dry_run_success');
    assert.equal(out.simulation, true);
    assert.deepEqual(out.result, {
      dryRun: true,
      wouldExecute: 'create_order',
      policyDomain: 'orders',
    });
    assert.equal(out.requestId, 'req-1');
    assert.equal(handlerCalls.length, 0);
  });

  it('requires every live nested write to cross the governed executor', async () => {
    const governedCalls = [];
    const { execute, handlerCalls } = makeExecutor({
      deps: {
        executeGovernedTool: async (toolName, params, options) => {
          governedCalls.push({ toolName, params, options });
          throw new Error(`Tool '${toolName}' is outside the governed kernel catalog.`);
        },
      },
    });

    const out = await execute({ ...base, dryRun: false, requestId: 'strict-plan' });
    assert.equal(out.status, 'error');
    assert.match(out.error, /outside the governed kernel catalog/);
    assert.equal(governedCalls.length, 1);
    assert.equal(governedCalls[0].toolName, 'create_order');
    assert.equal(governedCalls[0].options.requireGoverned, true);
    assert.equal(handlerCalls.length, 0);
  });

  it('executes the handler with the lazily-read tool context and runs after_tool_call', async () => {
    const { execute, handlerCalls, hookCalls } = makeExecutor();
    const out = await execute({
      ...base,
      requestId: 'req-2',
      sessionId: 'sess-2',
      extra: { k: 'v' },
    });
    assert.equal(out.status, 'success');
    assert.equal(out.resultSuccess, true);
    assert.equal(out.isRollback, false);
    assert.deepEqual(out.result, { id: 'ord_1', customerId: 'c1' });
    assert.match(out.resultHash, /^[0-9a-f]{64}$/);
    assert.equal(handlerCalls[0].commerce, 'ctx-commerce');
    assert.deepEqual(handlerCalls[0].extra, { requestId: 'req-2', sessionId: 'sess-2', k: 'v' });
    assert.deepEqual(
      hookCalls.map((h) => h.hook),
      ['before_tool_call', 'after_tool_call'],
    );
    assert.deepEqual(hookCalls[1].payload.result, { id: 'ord_1', customerId: 'c1' });
  });

  it('marks handler results carrying `error` as failed', async () => {
    const { execute } = makeExecutor({ handlerResult: { error: 'boom' } });
    const out = await execute(base);
    assert.equal(out.status, 'error');
    assert.equal(out.resultSuccess, false);
    assert.equal(out.error, 'boom');
    const rollback = await execute({ ...base, isRollback: true });
    assert.equal(rollback.status, 'rollback_failed');
  });

  it('maps rollback executions to rollback_success', async () => {
    const { execute } = makeExecutor();
    const out = await execute({ ...base, isRollback: true });
    assert.equal(out.status, 'rollback_success');
    assert.equal(out.isRollback, true);
  });

  it('catches thrown errors, notifies after_tool_call, and returns an error outcome', async () => {
    const { execute, hookCalls } = makeExecutor({
      deps: {
        wrapWithTelemetry: () => async () => {
          throw new Error('kaboom');
        },
      },
    });
    const out = await execute(base);
    assert.equal(out.status, 'error');
    assert.equal(out.error, 'kaboom');
    assert.equal(out.result, null);
    assert.equal(out.mutationManifest.phase, 'error');
    assert.equal(hookCalls.at(-1).hook, 'after_tool_call');
    assert.equal(hookCalls.at(-1).payload.error, 'kaboom');
  });
});
