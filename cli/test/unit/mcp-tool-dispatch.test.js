// Unit tests for cli/src/mcp/tool-dispatch.js
//
// Covers `createToolDispatch`:
//  - wrapTool: registers through the injected sdkTool; success path logs a
//    replay event (source=mcp_server) and returns the structured response;
//    hook / policy / permission / treasury gates short-circuit with the
//    matching status; thrown errors are logged and re-thrown
//  - executeTool: delegates to executeToolStepInPlan, logs a replay event
//    with source=embedded_agent_toolkit, and reports `success`
//  - executeToolWithPayment: routes through the MPP executor
//  - adaptTool: wraps handler output as MCP text content, including errors

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createToolDispatch } from '../../src/mcp/tool-dispatch.js';

function makeDispatch(overrides = {}) {
  const registered = [];
  const events = [];
  const hookCalls = [];
  const stepCalls = [];
  const dispatch = createToolDispatch({
    toolDomainByName: { create_order: 'orders' },
    inferPolicyDomain: () => 'inferred',
    getToolRuntimeMeta: (name) => ({
      name,
      permission: 'write',
      policyDomain: 'orders',
      sideEffect: 'write',
      compensations: [],
      idempotent: false,
    }),
    hookRunner: {
      hasHooks: () => true,
      run: async (hook, payload) => {
        hookCalls.push({ hook, payload });
        return overrides.hookResult?.[hook] ?? {};
      },
    },
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
    addAgenticReplayEvent: async (event) => {
      events.push(event);
    },
    buildToolResultResponse: (result, status, startedAt, toolMeta, isError) => ({
      built: true,
      result,
      status,
      toolMeta,
      isError,
    }),
    attachStructuredToolMetadataToResponse: (response, status, startedAt, toolMeta) => ({
      ...response,
      _structured: { status, name: toolMeta.name },
    }),
    executeToolStepInPlan: async (input) => {
      stepCalls.push(input);
      return {
        index: 0,
        tool: input.toolName,
        status: overrides.stepStatus ?? 'success',
        elapsedMs: 1,
        policy: { allowed: true, domain: 'orders' },
        permission: { allowed: true },
        charge: null,
        params: input.params,
        result: { ok: true },
        error: null,
      };
    },
    toolContext: { commerce: 'ctx' },
    sdkTool: (name, description, schema, handler) => {
      const entry = { name, description, schema, handler };
      registered.push(entry);
      return entry;
    },
    ...overrides.deps,
  });
  return { ...dispatch, registered, events, hookCalls, stepCalls };
}

describe('wrapTool', () => {
  it('registers through sdkTool and returns the structured success response', async () => {
    const d = makeDispatch();
    const wrapped = d.wrapTool('create_order', 'desc', { s: 1 }, async (args) => ({
      content: [{ type: 'text', text: JSON.stringify({ id: 'o1', args }) }],
    }));
    assert.equal(d.registered[0], wrapped);
    assert.equal(wrapped.name, 'create_order');

    const out = await wrapped.handler({ customerId: 'c1' }, { requestId: 'req-1' });
    assert.deepEqual(out._structured, { status: 'success', name: 'create_order' });
    assert.equal(out.content[0].type, 'text');

    assert.equal(d.events.length, 1);
    const evt = d.events[0];
    assert.equal(evt.tool, 'create_order');
    assert.equal(evt.status, 'success');
    assert.equal(evt.source, 'mcp_server');
    assert.equal(evt.agentic, true);
    assert.equal(evt.requestId, 'req-1');
    assert.equal(evt.policyDomain, 'inferred');
    assert.equal(evt.notes.mutationManifest.phase, 'success');
    assert.deepEqual(
      d.hookCalls.map((h) => h.hook),
      ['before_tool_call', 'after_tool_call'],
    );
  });

  it('takes sessionId from args when the transport does not provide one', async () => {
    const d = makeDispatch();
    const wrapped = d.wrapTool('create_order', 'desc', {}, async () => ({ content: [] }));
    await wrapped.handler({ sessionId: 'from-args' }, {});
    assert.equal(d.events[0].sessionId, 'from-args');
  });

  it('short-circuits on hook block, policy block, permission preview, and treasury block', async () => {
    const hook = makeDispatch({
      hookResult: { before_tool_call: { blocked: true, reason: 'hooked' } },
    });
    let out = await hook.wrapTool('t', 'd', {}, async () => ({})).handler({}, {});
    assert.equal(out.status, 'blocked');
    assert.equal(out.isError, true);
    assert.equal(out.result.error, 'hooked');
    assert.equal(hook.events[0].status, 'blocked');

    const policy = makeDispatch({
      deps: { evaluatePolicy: async () => ({ allowed: false, domain: 'orders', reason: 'pol' }) },
    });
    out = await policy.wrapTool('t', 'd', {}, async () => ({})).handler({}, {});
    assert.equal(out.status, 'policy_block');
    assert.equal(out.result.error, 'pol');
    assert.equal(policy.events[0].status, 'policy_block');

    const preview = makeDispatch({
      deps: {
        checkPermission: async () => ({
          allowed: false,
          preview: true,
          reason: 'pv',
          wouldDo: { a: 1 },
        }),
      },
    });
    out = await preview.wrapTool('t', 'd', {}, async () => ({})).handler({}, {});
    assert.equal(out.status, 'preview');
    assert.deepEqual(out.result.wouldDo, { a: 1 });
    assert.equal(preview.events[0].status, 'preview');

    const denied = makeDispatch({
      deps: { checkPermission: async () => ({ allowed: false, preview: false, reason: 'no' }) },
    });
    out = await denied.wrapTool('t', 'd', {}, async () => ({})).handler({}, {});
    assert.equal(out.status, 'permission_block');

    const treasury = makeDispatch({
      deps: {
        maybeChargeForTool: async () => ({ charged: false, blocked: true, reason: 'broke' }),
      },
    });
    out = await treasury.wrapTool('t', 'd', {}, async () => ({})).handler({}, {});
    assert.equal(out.status, 'treasury_block');
    assert.equal(out.result.error, 'broke');
  });

  it('returns payment_required for priced, unauthorized calls', async () => {
    const d = makeDispatch({
      deps: {
        resolveMppPaymentContext: async () => ({
          pricing: { amount: '1' },
          authorized: false,
          challenge: { challengeId: 'c' },
          errorPayload: { paymentRequired: true },
        }),
      },
    });
    const out = await d.wrapTool('t', 'd', {}, async () => ({})).handler({}, {});
    assert.equal(out.status, 'payment_required');
    assert.equal(out.result.paymentRequired, true);
    assert.equal(out.toolMeta.charge.paymentRequired, true);
    assert.equal(d.events[0].status, 'payment_required');
  });

  it('logs and re-throws handler errors', async () => {
    const d = makeDispatch();
    const wrapped = d.wrapTool('t', 'd', {}, async () => {
      throw new Error('boom');
    });
    await assert.rejects(() => wrapped.handler({}, {}), /boom/);
    assert.equal(d.events[0].status, 'error');
    assert.equal(d.events[0].error, 'boom');
    assert.equal(d.hookCalls.at(-1).payload.error, 'boom');
  });
});

describe('executeTool', () => {
  it('delegates to executeToolStepInPlan and logs an embedded-toolkit replay event', async () => {
    const d = makeDispatch();
    const out = await d.executeTool(
      'mcp__stateset-commerce__create_order',
      { a: 1 },
      { requestId: 'r1' },
    );
    assert.equal(out.success, true);
    assert.equal(out.requestId, 'r1');
    assert.equal(out.sessionId, 'r1');
    assert.equal(out.tool, 'create_order');
    assert.equal(d.stepCalls[0].toolName, 'create_order');
    assert.equal(d.stepCalls[0].includeHooks, true);
    assert.equal(d.stepCalls[0].stepIndex, 0);
    const evt = d.events[0];
    assert.equal(evt.source, 'embedded_agent_toolkit');
    assert.equal(evt.policyDomain, 'orders');
    assert.deepEqual(evt.notes, { directExecution: true, dryRun: false, includeHooks: true });
  });

  it('reports success=false for non-success statuses and generates ids', async () => {
    const d = makeDispatch({ stepStatus: 'policy_block' });
    const out = await d.executeTool('create_order');
    assert.equal(out.success, false);
    assert.match(out.requestId, /^[0-9a-f-]{36}$/);
  });
});

describe('executeToolWithPayment', () => {
  it('runs the tool through the MPP payment executor', async () => {
    const d = makeDispatch();
    const out = await d.executeToolWithPayment('create_order', { a: 1 }, { payment: {} });
    assert.equal(out.success, true);
    assert.equal(d.stepCalls.length, 1);
  });
});

describe('adaptTool', () => {
  it('bridges handler output to MCP text content, including thrown errors', async () => {
    const d = makeDispatch();
    const okTool = d.adaptTool({
      name: 'create_order',
      description: 'd',
      inputSchema: {},
      handler: async ({ commerce, params }) => ({ commerce, params }),
    });
    const ok = await okTool.handler({ x: 1 }, {});
    assert.deepEqual(JSON.parse(ok.content[0].text), { commerce: 'ctx', params: { x: 1 } });

    const badTool = d.adaptTool({
      name: 'create_order',
      description: 'd',
      inputSchema: {},
      handler: async () => {
        throw new Error('handler failed');
      },
    });
    const bad = await badTool.handler({}, {});
    assert.deepEqual(JSON.parse(bad.content[0].text), { success: false, error: 'handler failed' });
  });
});
