// Tests for the replay log's kernel block (cli/src/mcp/replay-kernel-fields.js)
// and its wiring in cli/src/mcp/tool-dispatch.js.
//
// A governed tool call's kernel receipt sits deep inside the replay event's
// `result`, where the sanitiser's depth limit truncated the parts an operator
// needs (policy reason codes, the principal). The event now carries a compact
// top-level `kernel` block drawn from the uncompacted receipt, bound into
// `resultHash` so the event signature covers it, and the kernel command
// carries a correlation id the replay event can be joined on.
//
//   - the extractor on applied, rejected and non-kernel results, and its bounds;
//   - resultHash binding, and correlation-id resolution;
//   - tool-dispatch wiring with stubbed deps (MCP path and executeTool path);
//   - the real server with a kernel policy: a denied call logs its reason codes
//     and principal, an applied call joins to its receipt, a read call has no
//     kernel block, every event's signature verifies, and an old-format event
//     from a fixture still verifies.

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { replayEventHash } from '../../src/mcp/audit-envelope.js';
import { signAuditArtifact } from '../../src/mcp/audit-signing.js';
import {
  MAX_KERNEL_FIELD_CHARS,
  MAX_KERNEL_REASON_CODES,
  bindKernelToResultHash,
  extractKernelReplayFields,
  resolveKernelCorrelationId,
} from '../../src/mcp/replay-kernel-fields.js';
import { createToolDispatch } from '../../src/mcp/tool-dispatch.js';
import {
  RAW_TOOL_RESULT,
  attachContractToResponse,
  normalizeToolResult,
} from '../../src/mcp/tool-result-contract.js';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

// Receipts shaped exactly as the native kernel returns them (captured from a
// real run; `result` elided).
const principal = {
  capabilities: ['customers.create', 'products.create'],
  delegated_by: 'user:u',
  id: 'agent:explore',
  kind: 'agent',
  tenant_id: 'tenant:t',
};
const appliedReceipt = () => ({
  aggregate_id: '8d1915f5-6266-405f-8055-8c3c8c03b6fe',
  aggregate_type: 'customer',
  audit_hash: '25036744ba37989f197d8b37402e222053d4b5b4f0877e820138d8c70a12fde5',
  command_id: '0c35698f-f315-41d5-a2ba-7fccabea3a1d',
  command_type: 'customers.create',
  completed_at: '2026-11-14T15:16:48.729284892Z',
  contract_version: '1.0',
  economic_context: {
    approval_id: null,
    authority_issuer: null,
    commitment: null,
    correlation_id: '2ef3bbda-9c62-4aed-ad7d-4c8f7422c892',
    mandate: null,
    principal,
    store_id: 'store:s',
  },
  error_code: null,
  error_message: null,
  event_ids: ['d029a652-a32b-4aa7-88ae-c913b6437d68'],
  idempotency_key: '42eb2c58-4bab-4e79-a953-30ba4f585b1a',
  policy: {
    allowed: true,
    decision_id: '44143e9f-b9b6-4fdb-9cc3-c2cf3687485a',
    policy_version: 'explore-v1',
    reason_codes: [],
  },
  receipt_id: '41aacb9d-3bbd-4959-b705-edea661e693c',
  result: { id: '8d1915f5-6266-405f-8055-8c3c8c03b6fe', email: 'a@example.com' },
  retry: 'same_key',
  started_at: '2026-11-14T15:16:48.728157812Z',
  status: 'succeeded',
  version_after: null,
  version_before: null,
});
const rejectedReceipt = () => ({
  aggregate_id: null,
  aggregate_type: 'product',
  audit_hash: 'd759d141f35d9839b938c618e6b305f9cc8628e82784fa3b13f84611f0c4002a',
  command_id: 'fa87a68a-f242-42fe-adcf-d3f2f26f05ef',
  command_type: 'products.create',
  completed_at: '2026-11-14T15:16:48.758091966Z',
  contract_version: '1.0',
  economic_context: {
    approval_id: null,
    authority_issuer: null,
    commitment: null,
    correlation_id: null,
    mandate: null,
    principal,
    store_id: 'store:s',
  },
  error_code: 'kernel.policy_denied',
  error_message: 'policy denied command: policy.approval_required',
  event_ids: [],
  idempotency_key: '5d57a67f-9ce2-435b-b09c-e38ab38ba243',
  policy: {
    allowed: false,
    decision_id: '915ba59f-4b3c-48f8-8284-3ed05ab60b3d',
    policy_version: 'explore-v1',
    reason_codes: ['policy.approval_required'],
  },
  receipt_id: '73f7ca99-6ba5-47c5-80c6-28a300aa66b7',
  result: null,
  retry: 'never',
  started_at: '2026-11-14T15:16:48.757189063Z',
  status: 'rejected',
  version_after: null,
  version_before: null,
});

/** The value createKernelToolExecutor returns for a governed call. */
const governedResult = (receipt) => ({
  success: receipt.status === 'succeeded' || receipt.status === 'previewed',
  kernel: true,
  commandType: receipt.command_type,
  preview: receipt.status === 'previewed',
  receipt,
  result: receipt.result ?? null,
});

/** The MCP CallToolResult adaptTool builds around a tool result. */
const mcpResponse = (toolResult) =>
  attachContractToResponse(
    { content: [{ type: 'text', text: JSON.stringify(toolResult, null, 2) }] },
    normalizeToolResult(toolResult),
  );

/**
 * Independent verifier for a persisted replay event: recompute `resultHash`
 * from the event's own `result` and `kernel`, then the event signature over
 * the payload cli/src/mcp/replay-log.js signs.
 */
function verifyReplayEvent(event, signing) {
  const resultHash = bindKernelToResultHash(
    replayEventHash(event.result || {}),
    event.kernel ?? null,
  );
  if (resultHash !== event.resultHash) return 'resultHash mismatch';
  const signaturePayload = {
    tool: event.tool || null,
    status: event.status || null,
    requestId: event.requestId || null,
    sessionId: event.sessionId || null,
    occurredAt: event.occurredAt || null,
    policyDomain: event.policyDomain || null,
    paramsHash: event.paramsHash,
    resultHash: event.resultHash,
    source: event.source || null,
  };
  if (signAuditArtifact(signaturePayload, signing).signature !== event.eventSignature) {
    return 'signature mismatch';
  }
  return 'ok';
}

describe('extractKernelReplayFields', () => {
  it('extracts the applied receipt from an MCP response', () => {
    const receipt = appliedReceipt();
    assert.deepEqual(extractKernelReplayFields(mcpResponse(governedResult(receipt))), {
      commandId: receipt.command_id,
      correlationId: receipt.economic_context.correlation_id,
      receiptStatus: 'succeeded',
      principalId: 'agent:explore',
      auditHash: receipt.audit_hash,
    });
  });

  it('extracts the rejected receipt with its error code and reason codes', () => {
    const receipt = rejectedReceipt();
    assert.deepEqual(extractKernelReplayFields(mcpResponse(governedResult(receipt))), {
      commandId: receipt.command_id,
      receiptStatus: 'rejected',
      errorCode: 'kernel.policy_denied',
      reasonCodes: ['policy.approval_required'],
      principalId: 'agent:explore',
      auditHash: receipt.audit_hash,
    });
  });

  it('reads a bare governed result, as executeTool sees it', () => {
    const block = extractKernelReplayFields(governedResult(rejectedReceipt()));
    assert.equal(block.receiptStatus, 'rejected');
    assert.deepEqual(block.reasonCodes, ['policy.approval_required']);
  });

  it('returns null for results that carry no governed receipt', () => {
    const notGoverned = [
      null,
      undefined,
      'text',
      42,
      [],
      {},
      { content: [{ type: 'text', text: '{"customers":[]}' }] },
      mcpResponse({ customers: [] }),
      // A payment receipt, not a kernel receipt: no `kernel` marker.
      mcpResponse({ success: true, receipt: { receipt_id: 'r-1', status: 'paid' } }),
      { kernel: true },
      { kernel: true, receipt: 'not-an-object' },
      { kernel: true, receipt: { command_id: 'c-1' } },
    ];
    for (const value of notGoverned) {
      assert.equal(extractKernelReplayFields(value), null, JSON.stringify(value));
    }
  });

  it('bounds reason codes and string sizes, and ignores non-string values', () => {
    const receipt = rejectedReceipt();
    receipt.command_id = 'c'.repeat(1000);
    receipt.economic_context.principal = { ...principal, id: 'p'.repeat(1000) };
    receipt.audit_hash = 42;
    receipt.policy.reason_codes = [
      ...Array.from({ length: 40 }, (_, i) => `policy.code_${i}_${'x'.repeat(500)}`),
      7,
      null,
    ];
    const block = extractKernelReplayFields(governedResult(receipt));
    assert.equal(block.reasonCodes.length, MAX_KERNEL_REASON_CODES);
    assert.equal(block.reasonCodesOmitted, 40 - MAX_KERNEL_REASON_CODES);
    for (const value of [block.commandId, block.principalId, ...block.reasonCodes]) {
      assert.ok(value.length <= MAX_KERNEL_FIELD_CHARS, `${value.length} chars`);
    }
    assert.ok(block.reasonCodes[0].startsWith('policy.code_0_'));
    assert.equal('auditHash' in block, false);
  });

  it('omits an empty reason-code list and fields the receipt does not have', () => {
    const block = extractKernelReplayFields(governedResult(appliedReceipt()));
    assert.equal('reasonCodes' in block, false);
    assert.equal('errorCode' in block, false);
    const bare = extractKernelReplayFields({ kernel: true, receipt: { status: 'failed' } });
    assert.deepEqual(bare, { receiptStatus: 'failed' });
  });
});

describe('bindKernelToResultHash', () => {
  const base = replayEventHash({ any: 'result' });

  it('leaves the result hash untouched when there is no kernel block', () => {
    assert.equal(bindKernelToResultHash(base, null), base);
    assert.equal(bindKernelToResultHash(base, undefined), base);
  });

  it('binds the kernel block deterministically and detects any change to it', () => {
    const kernel = { commandId: 'c-1', receiptStatus: 'rejected', reasonCodes: ['a', 'b'] };
    const bound = bindKernelToResultHash(base, kernel);
    assert.match(bound, /^[0-9a-f]{64}$/);
    assert.notEqual(bound, base);
    const reordered = { reasonCodes: ['a', 'b'], receiptStatus: 'rejected', commandId: 'c-1' };
    assert.equal(bindKernelToResultHash(base, reordered), bound);
    assert.notEqual(bindKernelToResultHash(base, { ...kernel, reasonCodes: ['a'] }), bound);
    assert.notEqual(bindKernelToResultHash(replayEventHash({ other: 1 }), kernel), bound);
  });
});

describe('resolveKernelCorrelationId', () => {
  it('uses a UUID request id, in the canonical lowercase form', () => {
    const id = randomUUID();
    assert.equal(resolveKernelCorrelationId(id), id);
    assert.equal(resolveKernelCorrelationId(id.toUpperCase()), id);
  });

  it('mints a UUID when the request id is not one (the kernel accepts only UUIDs)', () => {
    for (const requestId of [7, '7', 'req-1', '', null, undefined]) {
      assert.match(resolveKernelCorrelationId(requestId), UUID, String(requestId));
    }
    assert.equal(
      resolveKernelCorrelationId(7, () => 'minted'),
      'minted',
    );
  });
});

/** createToolDispatch with stubbed deps, recording replay events and kernel calls. */
function makeDispatch({ governed = null, stepResult = null } = {}) {
  const events = [];
  const governedCalls = [];
  const stepCalls = [];
  const dispatch = createToolDispatch({
    toolDomainByName: {},
    inferPolicyDomain: () => 'customers',
    getToolRuntimeMeta: (name) => ({
      name,
      permission: 'write',
      policyDomain: 'customers',
      sideEffect: 'write',
      compensations: [],
      idempotent: false,
    }),
    hookRunner: null,
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
    buildToolResultResponse: (result) => result,
    attachStructuredToolMetadataToResponse: (response) => response,
    executeToolStepInPlan: async (input) => {
      stepCalls.push(input);
      const raw = stepResult;
      return {
        index: 0,
        tool: input.toolName,
        status: 'success',
        elapsedMs: 1,
        policy: { allowed: true, domain: 'customers' },
        permission: { allowed: true },
        charge: null,
        params: input.params,
        result: raw,
        resultHash: replayEventHash(raw),
        error: null,
        [RAW_TOOL_RESULT]: raw,
      };
    },
    toolContext: {},
    executeGovernedTool: async (name, params, options) => {
      governedCalls.push({ name, params, options });
      return governed;
    },
    sdkTool: (name, description, schema, handler) => ({ name, handler }),
  });
  return { ...dispatch, events, governedCalls, stepCalls };
}

const adaptedTool = (d, handler = async () => ({ customers: [] })) =>
  d.adaptTool({
    name: 'create_customer',
    description: 'd',
    inputSchema: {},
    permission: 'write',
    handler,
  });

describe('tool-dispatch: MCP path', () => {
  it('attaches the kernel block of a governed call and binds it into resultHash', async () => {
    const d = makeDispatch({ governed: governedResult(rejectedReceipt()) });
    const requestId = randomUUID();
    await adaptedTool(d).handler({ email: 'a@example.com' }, { requestId, traceId: 'trace-1' });
    const [event] = d.events;
    assert.equal(event.status, 'error', 'the existing status is unchanged');
    assert.deepEqual(event.kernel.reasonCodes, ['policy.approval_required']);
    assert.equal(event.kernel.receiptStatus, 'rejected');
    assert.equal(event.kernel.principalId, 'agent:explore');
    assert.equal(
      event.resultHash,
      bindKernelToResultHash(replayEventHash(event.result || {}), event.kernel),
    );
    assert.notEqual(event.resultHash, replayEventHash(event.result || {}));
  });

  it('passes the UUID request id and the trace id to the kernel as correlation and trace', async () => {
    const d = makeDispatch({ governed: governedResult(appliedReceipt()) });
    const requestId = randomUUID();
    await adaptedTool(d).handler({}, { requestId, traceId: 'trace-1' });
    assert.equal(d.governedCalls[0].options.correlationId, requestId);
    assert.equal(d.governedCalls[0].options.traceId, 'trace-1');
    assert.equal(d.governedCalls[0].options.requestId, requestId);
  });

  it('mints a correlation UUID for a JSON-RPC numeric request id', async () => {
    const d = makeDispatch({ governed: governedResult(appliedReceipt()) });
    await adaptedTool(d).handler({}, { requestId: 7 });
    assert.match(d.governedCalls[0].options.correlationId, UUID);
  });

  it('prefers a correlation id the transport supplies', async () => {
    const d = makeDispatch({ governed: governedResult(appliedReceipt()) });
    const correlationId = randomUUID();
    await adaptedTool(d).handler({}, { requestId: randomUUID(), correlationId });
    assert.equal(d.governedCalls[0].options.correlationId, correlationId);
  });

  it('logs no kernel field, and the old resultHash, for an ungoverned call', async () => {
    const d = makeDispatch({ governed: null });
    await adaptedTool(d).handler({}, { requestId: randomUUID() });
    const [event] = d.events;
    assert.equal(event.status, 'success');
    assert.equal('kernel' in event, false);
    assert.equal(event.resultHash, replayEventHash(event.result || {}));
  });
});

describe('tool-dispatch: executeTool path', () => {
  it('attaches the kernel block and defaults the correlation id to the request id', async () => {
    const d = makeDispatch({ stepResult: governedResult(rejectedReceipt()) });
    const requestId = randomUUID();
    await d.executeTool('create_customer', {}, { requestId });
    assert.equal(d.stepCalls[0].extra.correlationId, requestId);
    const [event] = d.events;
    assert.equal(event.source, 'embedded_agent_toolkit');
    assert.deepEqual(event.kernel.reasonCodes, ['policy.approval_required']);
    assert.equal(
      event.resultHash,
      bindKernelToResultHash(replayEventHash(governedResult(rejectedReceipt())), event.kernel),
    );
  });

  it('keeps an explicit correlation id and logs no kernel field for an ungoverned call', async () => {
    const d = makeDispatch({ stepResult: { ok: true } });
    const correlationId = randomUUID();
    await d.executeTool('create_customer', {}, { requestId: 'r1', correlationId });
    assert.equal(d.stepCalls[0].extra.correlationId, correlationId);
    assert.equal('kernel' in d.events[0], false);
  });
});

describe('the replay log on the real server with a kernel policy', () => {
  const SIGNING = { signingKey: 'replay-kernel-test-key', keyId: 'replay-kernel-test' };
  const ENV_KEYS = ['STATESET_AGENTIC_AUDIT_SIGNING_KEY', 'STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID'];
  const savedEnv = {};
  const commandPolicy = (capability, overrides = {}) => ({
    required_capabilities: [capability],
    requires_approval: false,
    requires_tenant: true,
    requires_store: true,
    allowed_tenant_ids: ['tenant:replay'],
    allowed_store_ids: ['store:replay'],
    requires_agent_delegation: true,
    requires_signed_authority: false,
    ...overrides,
  });
  let dir;
  let server;
  const calls = {};

  const tool = (name) => {
    const found = server.getAdaptedTools().find((t) => t.name === name);
    assert.ok(found, `${name} is exposed`);
    return found;
  };
  const eventFor = async (requestId) => {
    const { events } = await server.getReplayLog({ requestId, limit: 10 });
    assert.equal(events.length, 1, `one replay event for request ${requestId}`);
    return events[0];
  };

  before(async () => {
    for (const key of ENV_KEYS) savedEnv[key] = process.env[key];
    process.env.STATESET_AGENTIC_AUDIT_SIGNING_KEY = SIGNING.signingKey;
    process.env.STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID = SIGNING.keyId;
    dir = mkdtempSync(path.join(os.tmpdir(), 'replay-kernel-'));
    server = createStatesetMcpServer({
      dbPath: path.join(dir, 'store.db'),
      allowApply: true,
      kernel: {
        strict: true,
        storeId: 'store:replay',
        principal: {
          id: 'agent:replay',
          kind: 'agent',
          tenantId: 'tenant:replay',
          delegatedBy: 'user:replay',
          capabilities: ['customers.create', 'products.create'],
        },
        policy: {
          version: 'replay-v1',
          commands: {
            'customers.create': commandPolicy('customers.create'),
            // Product creation needs an approval this agent never carries.
            'products.create': commandPolicy('products.create', { requires_approval: true }),
          },
          trusted_authority_keys: {},
        },
      },
    });

    calls.applied = { requestId: randomUUID() };
    calls.applied.response = await tool('create_customer').handler(
      { email: 'ada@example.com', firstName: 'Ada', lastName: 'Lovelace' },
      { requestId: calls.applied.requestId },
    );
    calls.denied = { requestId: randomUUID() };
    calls.denied.response = await tool('create_product').handler(
      { name: 'Widget', variants: [{ sku: 'W-1', price: '5.00' }] },
      { requestId: calls.denied.requestId },
    );
    calls.read = { requestId: randomUUID() };
    calls.read.response = await tool('list_customers').handler(
      {},
      { requestId: calls.read.requestId },
    );
    // stdio clients number their requests; the kernel takes only UUIDs.
    calls.numeric = { requestId: 4242 };
    calls.numeric.response = await tool('create_customer').handler(
      { email: 'grace@example.com', firstName: 'Grace', lastName: 'Hopper' },
      { requestId: calls.numeric.requestId },
    );
  });

  after(async () => {
    for (const key of ENV_KEYS) {
      if (savedEnv[key] === undefined) delete process.env[key];
      else process.env[key] = savedEnv[key];
    }
    try {
      await server?.close?.();
    } catch {
      // never connected to a transport
    }
    await server?.dispose?.();
    rmSync(dir, { recursive: true, force: true });
  });

  const receiptOf = (call) => call.response.structuredContent.result.receipt;

  it('logs a denied call with its reason codes, error code and principal', async () => {
    const receipt = receiptOf(calls.denied);
    assert.equal(receipt.status, 'rejected');
    const event = await eventFor(calls.denied.requestId);
    assert.equal(event.status, 'error', 'the existing status is unchanged');
    assert.equal(event.kernel.receiptStatus, 'rejected');
    assert.ok(event.kernel.reasonCodes.length > 0);
    assert.ok(event.kernel.reasonCodes.includes('policy.approval_required'));
    assert.equal(event.kernel.errorCode, 'kernel.policy_denied');
    assert.equal(event.kernel.principalId, 'agent:replay');
    assert.equal(event.kernel.commandId, receipt.command_id);
    assert.equal(event.kernel.auditHash, receipt.audit_hash);
    assert.equal(event.kernel.correlationId, event.requestId);
    // The existing (compacted) result is still logged as before.
    assert.equal(event.result.structuredContent.result.receipt.status, 'rejected');
  });

  it('joins an applied call to its receipt by command id and correlation id', async () => {
    const receipt = receiptOf(calls.applied);
    assert.equal(receipt.status, 'succeeded');
    const event = await eventFor(calls.applied.requestId);
    assert.equal(event.status, 'success');
    assert.equal(event.kernel.receiptStatus, 'succeeded');
    assert.equal(event.kernel.commandId, receipt.command_id);
    assert.equal(event.kernel.correlationId, event.requestId);
    // The correlation id reached the kernel command envelope.
    assert.equal(receipt.economic_context.correlation_id, calls.applied.requestId);
    assert.equal(event.kernel.principalId, 'agent:replay');
    assert.equal('reasonCodes' in event.kernel, false);
  });

  it('gives a numerically identified call a minted correlation id the receipt carries', async () => {
    const receipt = receiptOf(calls.numeric);
    const event = await eventFor(calls.numeric.requestId);
    assert.match(event.kernel.correlationId, UUID);
    assert.equal(event.kernel.correlationId, receipt.economic_context.correlation_id);
  });

  it('logs no kernel field for a read call', async () => {
    const event = await eventFor(calls.read.requestId);
    assert.equal(event.status, 'success');
    assert.equal('kernel' in event, false);
    assert.equal(event.resultHash, replayEventHash(event.result || {}));
  });

  it('signs every new event so that it verifies, kernel block included', async () => {
    for (const call of Object.values(calls)) {
      const event = await eventFor(call.requestId);
      assert.equal(verifyReplayEvent(event, SIGNING), 'ok', `${event.tool} ${event.requestId}`);
    }
  });

  it('fails verification when the kernel block is altered or removed', async () => {
    const event = await eventFor(calls.denied.requestId);
    const altered = structuredClone(event);
    altered.kernel.reasonCodes = [];
    assert.equal(verifyReplayEvent(altered, SIGNING), 'resultHash mismatch');
    const stripped = structuredClone(event);
    delete stripped.kernel;
    assert.equal(verifyReplayEvent(stripped, SIGNING), 'resultHash mismatch');
  });

  it('still verifies an old-format event written before the kernel block existed', () => {
    const line = readFileSync(
      path.join(HERE, '../fixtures/replay/legacy-governed-denied-event.jsonl'),
      'utf8',
    ).trim();
    const legacy = JSON.parse(line);
    assert.equal('kernel' in legacy, false);
    assert.equal(legacy.result.structuredContent.result.receipt.policy, '[truncated]');
    assert.equal(
      verifyReplayEvent(legacy, { signingKey: 'replay-fixture-key', keyId: 'replay-fixture' }),
      'ok',
    );
  });
});
