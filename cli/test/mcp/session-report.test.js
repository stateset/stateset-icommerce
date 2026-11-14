// Tests for cli/src/mcp/session-report.js and bin/stateset-agent-report.js.
//
// The fixture events are the shapes the real server writes (captured from
// `createStatesetMcpServer` with a kernel config and with treasury pricing),
// trimmed of hashes and the policy decision bundle:
//   - `mcp_server` events wrap the tool result in `result.structuredContent`,
//     and the sanitizer's depth bound leaves the receipt's `policy` as the
//     string "[truncated]";
//   - `embedded_agent_toolkit` events hold the tool result directly, so the
//     receipt keeps `policy.reason_codes`;
//   - a kernel refusal is logged with `status: "error"`;
//   - a charge is `{ charged, rule: { tool, chainId, tokenSymbol, amount } }`,
//     the amount being whatever JSON type the pricing rule used.

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

import {
  UNSESSIONED,
  buildSessionReport,
  buildSessionReportFromFile,
  classifyEvent,
  createSessionReportBuilder,
  findKernelReceipt,
  formatSessionReport,
  readReplayLogEvents,
} from '../../src/mcp/session-report.js';

const exec = promisify(execFile);
const BIN = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'bin',
  'stateset-agent-report.js',
);

let seq = 0;
const base = (overrides) => {
  seq += 1;
  return {
    eventId: `evt-${seq}`,
    requestId: `req-${seq}`,
    policyDomain: 'orders',
    occurredAt: `2026-11-14T11:28:${String(seq).padStart(2, '0')}.000Z`,
    params: {},
    policy: { allowed: true, domain: 'orders', actions: [] },
    permission: { allowed: true },
    charge: { charged: false },
    error: null,
    notes: { mutationManifest: null },
    source: 'mcp_server',
    agentic: true,
    eventSignature: 'sig',
    ...overrides,
  };
};

const receipt = (overrides) => ({
  aggregate_type: 'customer',
  command_type: 'customers.create',
  contract_version: '1.0',
  error_code: null,
  error_message: null,
  receipt_id: 'd14fe523-f4f8-4ea2-acfe-48640e5e4f55',
  retry: 'never',
  status: 'succeeded',
  ...overrides,
});

const mcpResult = (contract) => ({
  content: [{ type: 'text', text: '{...}' }],
  structuredContent: contract,
});

const EVENTS = [
  // sess-A: an ok read
  base({
    tool: 'list_customers',
    status: 'success',
    sessionId: 'sess-A',
    elapsedMs: 10,
    result: mcpResult({ ok: true, preview: false, result: { customers: [] } }),
  }),
  // sess-A: an ok write, charged, amount as a decimal string
  base({
    tool: 'create_order',
    status: 'success',
    sessionId: 'sess-A',
    elapsedMs: 20,
    charge: {
      charged: true,
      rule: { tool: 'create_order', chainId: 'base', tokenSymbol: 'USDC', amount: '0.1' },
      mpp: null,
    },
    result: mcpResult({
      ok: true,
      preview: false,
      result: {
        success: true,
        kernel: true,
        receipt: receipt({
          command_type: 'orders.create',
          economic_context: '[truncated]',
          policy: '[truncated]',
        }),
      },
    }),
  }),
  // sess-A: a charged read, amount as a JSON number (as a pricing.json may hold)
  base({
    tool: 'list_orders',
    status: 'success',
    sessionId: 'sess-A',
    elapsedMs: 30,
    charge: {
      charged: true,
      rule: { tool: 'list_orders', chainId: 'base', tokenSymbol: 'USDC', amount: 0.2 },
      mpp: null,
    },
    result: mcpResult({ ok: true, preview: false, result: { orders: [] } }),
  }),
  // sess-A: kernel policy denial with reason codes (executeTool shape)
  base({
    tool: 'create_customer',
    status: 'error',
    sessionId: 'sess-A',
    elapsedMs: 40,
    source: 'embedded_agent_toolkit',
    error: 'policy denied command: policy.capability_missing:customers.create',
    notes: { directExecution: true, dryRun: false, includeHooks: true },
    result: {
      success: false,
      kernel: true,
      commandType: 'customers.create',
      preview: false,
      receipt: receipt({
        status: 'rejected',
        error_code: 'kernel.policy_denied',
        error_message: 'policy denied command: policy.capability_missing:customers.create',
        economic_context: {
          principal: { id: 'agent:contract', kind: 'agent', tenant_id: 'tenant:contract' },
          store_id: 'store:contract',
        },
        policy: {
          allowed: false,
          decision_id: '5092bd96-f2c8-40e3-bf0c-4cd6cce8a087',
          policy_version: 'contract-v1',
          reason_codes: ['policy.capability_missing:customers.create', 'policy.approval_required'],
        },
      }),
      result: null,
    },
  }),
  // sess-A: the same denial through the MCP tool: policy truncated by the sanitizer
  base({
    tool: 'create_customer',
    status: 'error',
    sessionId: 'sess-A',
    elapsedMs: 50,
    result: {
      ...mcpResult({
        ok: false,
        preview: false,
        error: {
          code: 'kernel.policy_denied',
          message: 'policy denied command: policy.capability_missing:customers.create',
          retryable: false,
        },
        result: {
          success: false,
          kernel: true,
          receipt: receipt({
            status: 'rejected',
            error_code: 'kernel.policy_denied',
            error_message: 'policy denied command: policy.capability_missing:customers.create',
            economic_context: '[truncated]',
            policy: '[truncated]',
          }),
          result: null,
        },
      }),
      isError: true,
    },
  }),
  // sess-A: a permission block
  base({
    tool: 'cancel_order',
    status: 'permission_block',
    sessionId: 'sess-A',
    elapsedMs: 5,
    permission: { allowed: false, reason: 'Permission denied' },
    charge: null,
    error: 'Permission denied',
  }),
  // sess-A: a tool error with a binding code
  base({
    tool: 'get_payment',
    status: 'error',
    sessionId: 'sess-A',
    elapsedMs: 100,
    result: {
      ...mcpResult({
        ok: false,
        preview: false,
        error: { code: 'VALIDATION', message: 'Invalid UUID', retryable: false },
      }),
      isError: true,
    },
  }),
  // sess-B: a policy block, a thrown error with no code, and a BTC charge
  base({
    tool: 'create_refund',
    status: 'policy_block',
    sessionId: 'sess-B',
    elapsedMs: 2,
    policy: { domain: 'payments', actions: [], explanations: [] },
    permission: null,
    charge: null,
    error: 'Tool execution blocked by policy',
  }),
  base({
    tool: 'get_order',
    status: 'invalid',
    sessionId: 'sess-B',
    elapsedMs: 1,
    source: 'embedded_agent_toolkit',
    result: null,
    policy: null,
    permission: null,
    charge: null,
    error: "Invalid parameters for tool 'get_order'",
  }),
  base({
    tool: 'list_customers',
    status: 'success',
    sessionId: 'sess-B',
    elapsedMs: 7,
    charge: {
      charged: true,
      rule: { tool: 'list_customers', chainId: 'bitcoin', tokenSymbol: 'BTC', amount: 0.0001 },
      mpp: { challengeId: 'mpp_1', credentialId: 'c-1', paymentMethod: 'bitcoin' },
    },
    result: mcpResult({ ok: true, preview: false, result: { customers: [] } }),
  }),
  // unsessioned: a preview
  base({
    tool: 'create_customer',
    status: 'preview',
    sessionId: null,
    requestId: null,
    elapsedMs: 1,
    permission: { allowed: false, preview: true, reason: 'Preview mode' },
    charge: null,
    error: "Preview mode: would execute 'create_customer' if --apply flag is set",
  }),
];

// Events as written once tool-dispatch.js lifts a top-level `kernel` block out
// of the UNCOMPACTED receipt (cli/src/mcp/replay-kernel-fields.js on
// feat/replay-kernel-correlation): { commandId, correlationId, receiptStatus,
// errorCode, reasonCodes, reasonCodesOmitted, principalId, auditHash }, each
// present only when the receipt had it, absent on ungoverned calls. The
// receipt inside `result` is still sanitised exactly as before, so on the MCP
// path its `policy` and `economic_context` remain "[truncated]".
const truncatedGoverned = (receiptOverrides, contract) =>
  mcpResult({
    preview: false,
    ...contract,
    result: {
      success: receiptOverrides.status === 'succeeded',
      kernel: true,
      receipt: receipt({
        economic_context: '[truncated]',
        policy: '[truncated]',
        ...receiptOverrides,
      }),
      result: null,
    },
  });

const kernelBlock = (overrides) => ({
  commandId: `cmd-${seq + 1}`,
  correlationId: '6a0c3c53-3f4e-4f43-9c51-6d2a1a7c0b11',
  auditHash: '4c73427182458cf83dfe40aec418483de2c204f25b329539d832d17e1e6e3ee5',
  principalId: 'agent:explore',
  ...overrides,
});

const APPROVAL_DENIED = 'policy denied command: policy.approval_required';

const KERNEL_EVENTS = [
  // applied: the principal survives only in the kernel block
  base({
    tool: 'create_customer',
    status: 'success',
    sessionId: 'sess-K',
    elapsedMs: 12,
    kernel: kernelBlock({ receiptStatus: 'succeeded' }),
    result: truncatedGoverned({ status: 'succeeded' }, { ok: true }),
  }),
  // denied with reason codes; the receipt's policy is truncated (MCP path)
  base({
    tool: 'create_refund',
    status: 'error',
    sessionId: 'sess-K',
    elapsedMs: 8,
    kernel: kernelBlock({
      receiptStatus: 'rejected',
      errorCode: 'kernel.policy_denied',
      reasonCodes: ['policy.approval_required', 'policy.max_amount_exceeded'],
    }),
    result: {
      ...truncatedGoverned(
        { status: 'rejected', error_code: 'kernel.policy_denied', error_message: APPROVAL_DENIED },
        {
          ok: false,
          error: { code: 'kernel.policy_denied', message: APPROVAL_DENIED, retryable: false },
        },
      ),
      isError: true,
    },
  }),
  // denied on the executeTool path: block AND full receipt -- counted once
  base({
    tool: 'create_refund',
    status: 'error',
    sessionId: 'sess-K',
    elapsedMs: 6,
    source: 'embedded_agent_toolkit',
    error: APPROVAL_DENIED,
    kernel: kernelBlock({
      receiptStatus: 'rejected',
      errorCode: 'kernel.policy_denied',
      reasonCodes: ['policy.approval_required'],
    }),
    result: {
      success: false,
      kernel: true,
      receipt: receipt({
        status: 'rejected',
        error_code: 'kernel.policy_denied',
        error_message: APPROVAL_DENIED,
        economic_context: { principal: { id: 'agent:explore', kind: 'agent' } },
        policy: { allowed: false, reason_codes: ['policy.approval_required'] },
      }),
      result: null,
    },
  }),
  // a domain rejection: the receipt had no reason codes, so the block has none
  base({
    tool: 'ship_order',
    status: 'error',
    sessionId: 'sess-K',
    elapsedMs: 4,
    kernel: kernelBlock({
      receiptStatus: 'rejected',
      errorCode: 'commerce.invalid_order_status_transition',
    }),
    result: truncatedGoverned(
      { status: 'rejected', error_code: 'commerce.invalid_order_status_transition' },
      { ok: false, error: { code: 'commerce.invalid_order_status_transition', message: 'x' } },
    ),
  }),
  // a failed command whose result did not survive: only the block names it
  base({
    tool: 'capture_payment',
    status: 'error',
    sessionId: 'sess-K',
    elapsedMs: 3,
    kernel: kernelBlock({ receiptStatus: 'failed', errorCode: 'kernel.storage' }),
    result: null,
  }),
  // the writer keeps 16 reason codes and counts the rest
  base({
    tool: 'create_payment',
    status: 'error',
    sessionId: 'sess-cap',
    elapsedMs: 9,
    kernel: kernelBlock({
      receiptStatus: 'rejected',
      errorCode: 'kernel.policy_denied',
      reasonCodes: Array.from(
        { length: 16 },
        (_, i) => `policy.code_${String(i).padStart(2, '0')}`,
      ),
      reasonCodesOmitted: 3,
    }),
    result: truncatedGoverned(
      { status: 'rejected', error_code: 'kernel.policy_denied' },
      { ok: false, error: { code: 'kernel.policy_denied', message: 'denied' } },
    ),
  }),
];

// One session spanning the upgrade: an old-format refusal (reason codes
// truncated away) and a new-format refusal carrying a kernel block. The copy
// gets its own commandId: the same commandId again would be a retry.
const MIXED_SESSION = [
  { ...EVENTS[4], eventId: 'evt-mixed-old', sessionId: 'sess-M' },
  {
    ...KERNEL_EVENTS[1],
    eventId: 'evt-mixed-new',
    sessionId: 'sess-M',
    kernel: { ...KERNEL_EVENTS[1].kernel, commandId: 'cmd-mixed' },
  },
];

const NO_OUTCOMES = Object.freeze({
  executed: 0,
  previewed: 0,
  dryRun: 0,
  ok: 0,
  denied: 0,
  error: 0,
  replayed: 0,
  other: 0,
});
const outcomes = (counts) => ({ ...NO_OUTCOMES, ...counts });

/** A session of a report, by id. */
const sessionOf = (report, id) => report.sessions.find((s) => s.sessionId === id);

// A plan run through MCP, as plan-executor.js and tool-dispatch.js write it:
// one event per step (the step's tool in notes.sourceStep / params.step, the
// step outcome in `result`), the plan's own record (notes.final), then the
// MCP wrapper event, whose tool result names the plan's requestId.
const PLAN_REQUEST = '0f6c1a52-6a43-4c4f-9e57-0d1a8c1f9a10';
const planEvent = (overrides) =>
  base({
    tool: 'agentic_execute_plan',
    source: 'agentic_execute_plan',
    requestId: PLAN_REQUEST,
    sessionId: 'sess-P',
    policyDomain: 'agentic',
    ...overrides,
  });
const planStep = (index, tool, overrides = {}) =>
  planEvent({
    status: 'success',
    elapsedMs: 10,
    params: { step: tool, params: {}, resolved: true, source: { step: index } },
    notes: {
      dryRun: false,
      executedBy: 'agentic_execute_plan',
      index,
      sourceStep: tool,
      mutationManifest: { version: '1.0.0', tool, sideEffect: 'write', phase: 'success' },
    },
    ...overrides,
  });

// A governed step refused by the kernel: the receipt is at
// result.result.receipt, and a second sanitising pass has cut its reason codes.
const REFUSED_STEP_RESULT = {
  index: 1,
  tool: 'create_inventory_item',
  status: 'error',
  result: {
    success: false,
    kernel: true,
    commandType: 'inventory.item.create',
    receipt: receipt({
      command_id: '68d049e5-81b9-48e9-9129-0bca2f91bdc7',
      command_type: 'inventory.item.create',
      status: 'rejected',
      error_code: 'kernel.policy_denied',
      economic_context: { principal: '[truncated]', store_id: 'store:p' },
      policy: { allowed: false, policy_version: 'p1', reason_codes: '[truncated]' },
    }),
    result: null,
  },
  error: 'policy denied command: policy.capability_missing:inventory.item.create',
};

const PLAN_EVENTS = [
  planStep(0, 'create_customer', {
    result: {
      index: 0,
      tool: 'create_customer',
      status: 'success',
      result: {
        success: true,
        kernel: true,
        receipt: receipt({ command_id: 'b7b1d0f2-5f0c-4b8e-9a43-3c8c6e1f0a01' }),
      },
    },
  }),
  planStep(1, 'create_inventory_item', {
    status: 'error',
    error: REFUSED_STEP_RESULT.error,
    result: REFUSED_STEP_RESULT,
  }),
  planEvent({
    status: 'failed',
    elapsedMs: 40,
    params: { dryRun: false, totalSteps: 2, completedSteps: 1, failedSteps: 1 },
    result: { finalStatus: 'failed', stepStatuses: ['success', 'error'] },
    charge: null,
    notes: { final: true, planSignature: 'p', executionSignature: 'e' },
  }),
  base({
    tool: 'agentic_execute_plan',
    status: 'success',
    sessionId: 'sess-P',
    requestId: null,
    elapsedMs: 45,
    charge: {
      charged: true,
      rule: { tool: 'agentic_execute_plan', chainId: 'base', tokenSymbol: 'USDC', amount: '0.5' },
    },
    notes: { mutationManifest: { tool: 'agentic_execute_plan', sideEffect: 'write' } },
    result: mcpResult({
      ok: true,
      preview: false,
      result: { tool: 'agentic_execute_plan', requestId: PLAN_REQUEST, finalStatus: 'failed' },
    }),
  }),
];

let dir;
let logPath;
let mixedLogPath;

before(() => {
  dir = mkdtempSync(path.join(os.tmpdir(), 'session-report-'));
  logPath = path.join(dir, 'agentic-tool-calls.jsonl');
  const lines = EVENTS.map((event) => JSON.stringify(event));
  // A torn write and a blank line in the middle of the log.
  lines.splice(3, 0, '{"eventId":"evt-torn","tool":"list_cust', '');
  writeFileSync(logPath, `${lines.join('\n')}\n`);
  mixedLogPath = path.join(dir, 'mixed.jsonl');
  writeFileSync(
    mixedLogPath,
    `${[...EVENTS, ...KERNEL_EVENTS, ...MIXED_SESSION].map((e) => JSON.stringify(e)).join('\n')}\n`,
  );
});

after(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe('buildSessionReport', () => {
  const report = buildSessionReport(EVENTS);
  const a = sessionOf(report, 'sess-A');
  const b = sessionOf(report, 'sess-B');

  it('groups calls by session in first-seen order, with one bucket for the unsessioned', () => {
    assert.deepEqual(
      report.sessions.map((s) => s.sessionId),
      ['sess-A', 'sess-B', UNSESSIONED],
    );
    assert.deepEqual(
      report.sessions.map((s) => s.unsessioned),
      [false, false, true],
    );
    assert.equal(report.sessionCount, 3);
    assert.equal(a.calls, 7);
    assert.equal(b.calls, 3);
    assert.equal(sessionOf(report, UNSESSIONED).calls, 1);
    assert.equal(report.totals.calls, 11);
    assert.equal(report.totals.steps, 0);
    assert.equal(report.malformedLines, 0);
  });

  it('counts by status, tool and source', () => {
    assert.deepEqual(a.byStatus, { error: 3, success: 3, permission_block: 1 });
    assert.deepEqual(a.byTool, {
      create_customer: 2,
      cancel_order: 1,
      create_order: 1,
      get_payment: 1,
      list_customers: 1,
      list_orders: 1,
    });
    assert.deepEqual(a.bySource, { mcp_server: 6, embedded_agent_toolkit: 1 });
    assert.deepEqual(report.totals.byStatus, {
      success: 4,
      error: 3,
      invalid: 1,
      permission_block: 1,
      policy_block: 1,
      preview: 1,
    });
  });

  it('separates committed writes, previews, refusals and errors', () => {
    // create_order's receipt succeeded: executed; the two reads are ok.
    assert.deepEqual(a.outcomes, outcomes({ executed: 1, ok: 2, denied: 3, error: 1 }));
    assert.deepEqual(b.outcomes, outcomes({ ok: 1, denied: 1, error: 1 }));
    assert.deepEqual(sessionOf(report, UNSESSIONED).outcomes, outcomes({ previewed: 1 }));
  });

  it('groups denials by kernel reason code, else error_code, else the gate outcome', () => {
    assert.equal(a.denials.total, 3);
    assert.deepEqual(a.denials.byReason, {
      // from receipt.policy.reason_codes (one denial, two codes)
      'policy.approval_required': 1,
      'policy.capability_missing:customers.create': 1,
      // reason codes truncated out of the mcp_server event: receipt.error_code
      'kernel.policy_denied': 1,
      // no machine-readable code exists for a permission gate
      permission_block: 1,
    });
    assert.equal(a.denials.reasonCodesUnavailable, 1);
    assert.deepEqual(b.denials.byReason, { policy_block: 1 });
    assert.equal(report.totals.denials.total, 4);
  });

  it('counts errors by structured code and never parses one from prose', () => {
    assert.deepEqual(a.errors, { total: 1, byCode: { VALIDATION: 1 } });
    assert.deepEqual(b.errors, { total: 1, byCode: { '(no code) invalid': 1 } });
  });

  it('sums charges exactly, per token, and says when amounts were numbers', () => {
    assert.notEqual(0.1 + 0.2, 0.3, 'the float sum this must not reproduce');
    assert.deepEqual(a.charges, {
      count: 2,
      byToken: [{ tokenSymbol: 'USDC', chainId: 'base', count: 2, total: '0.30' }],
      onReplayedReceipts: 0,
      numericAmounts: 1,
      unparseableAmounts: 0,
    });
    assert.deepEqual(b.charges.byToken, [
      { tokenSymbol: 'BTC', chainId: 'bitcoin', count: 1, total: '0.0001' },
    ]);
    assert.deepEqual(
      report.totals.charges.byToken.map((t) => `${t.total} ${t.tokenSymbol}`),
      ['0.0001 BTC', '0.30 USDC'],
    );
    assert.equal(report.totals.charges.numericAmounts, 2);
  });

  it('keeps a long run of small charges exact', () => {
    const many = Array.from({ length: 1000 }, () =>
      base({
        tool: 't',
        status: 'success',
        sessionId: 's',
        elapsedMs: 1,
        charge: { charged: true, rule: { chainId: 'base', tokenSymbol: 'USDC', amount: '0.1' } },
      }),
    );
    assert.equal(buildSessionReport(many).totals.charges.byToken[0].total, '100.00');
  });

  it('does not total a charge it cannot read, and says so', () => {
    const r = buildSessionReport([
      base({
        tool: 't',
        status: 'success',
        sessionId: 's',
        elapsedMs: 1,
        charge: { charged: true, rule: { chainId: 'base', tokenSymbol: 'USDC', amount: 'lots' } },
      }),
    ]);
    assert.equal(r.totals.charges.count, 1);
    assert.equal(r.totals.charges.unparseableAmounts, 1);
    assert.deepEqual(r.totals.charges.byToken, []);
  });

  it('reports first/last timestamps and nearest-rank latency percentiles', () => {
    assert.equal(a.firstAt, EVENTS[0].occurredAt);
    assert.equal(a.lastAt, EVENTS[6].occurredAt);
    // sess-A elapsedMs sorted: 5 10 20 30 40 50 100
    assert.deepEqual(a.latencyMs, { count: 7, p50: 30, p95: 100, max: 100 });
    // sess-B: 1 2 7
    assert.deepEqual(b.latencyMs, { count: 3, p50: 2, p95: 7, max: 7 });
    // all: 1 1 2 5 7 10 20 30 40 50 100
    assert.deepEqual(report.totals.latencyMs, { count: 11, p50: 10, p95: 100, max: 100 });
    assert.equal(report.totals.firstAt, EVENTS[0].occurredAt);
    assert.equal(report.totals.lastAt, EVENTS[10].occurredAt);
  });

  it('names the principal only where the receipt kept it', () => {
    assert.deepEqual(a.principals, ['agent:contract']);
    assert.deepEqual(b.principals, []);
  });

  it('counts entries that are not events and never throws on them', () => {
    const r = buildSessionReport([
      null,
      42,
      'x',
      [],
      { _parseError: 'Unexpected end', raw: '{' },
      {},
      EVENTS[0],
    ]);
    assert.equal(r.malformedLines, 5);
    assert.equal(r.totals.calls, 2);
    assert.equal(sessionOf(r, UNSESSIONED).byStatus['(unknown)'], 1);
    assert.deepEqual(sessionOf(r, UNSESSIONED).latencyMs, {
      count: 0,
      p50: null,
      p95: null,
      max: null,
    });
  });

  it('filters to one session', () => {
    const r = buildSessionReport(EVENTS, { sessionId: 'sess-B' });
    assert.deepEqual(
      r.sessions.map((s) => s.sessionId),
      ['sess-B'],
    );
    assert.equal(r.totals.calls, 3);
    assert.equal(r.filteredOut, 8);
    assert.equal(buildSessionReport(EVENTS, { sessionId: UNSESSIONED }).totals.calls, 1);
  });
});

describe('unsessioned calls', () => {
  // The embedded toolkit's default: sessionId = requestId, one per call.
  const toolkitCall = (i, overrides = {}) => ({
    eventId: `evt-tk-${i}`,
    tool: 'get_order',
    status: 'success',
    requestId: `req-tk-${i}`,
    sessionId: `req-tk-${i}`,
    occurredAt: '2026-11-14T12:00:00.000Z',
    elapsedMs: i % 50,
    result: { order: null },
    charge: { charged: false },
    notes: { directExecution: true, dryRun: false, includeHooks: true },
    source: 'embedded_agent_toolkit',
    ...overrides,
  });

  it('puts 100k per-call pseudo-sessions in one bucket', () => {
    const builder = createSessionReportBuilder();
    const N = 100_000;
    for (let i = 0; i < N; i += 1) {
      builder.add(toolkitCall(i, i % 1000 === 999 ? { sessionId: `sess-${i % 3}` } : {}));
    }
    const r = builder.finish();
    // the unsessioned bucket plus the three real sessions
    assert.equal(r.sessionCount, 4);
    assert.equal(r.sessions[0].sessionId, UNSESSIONED);
    assert.equal(r.sessions[0].calls, N - N / 1000);
    assert.equal(r.totals.calls, N);
    assert.equal(r.totals.latencyMs.count, N);
  });

  it('treats a missing, blank or request-id session id as unsessioned', () => {
    const r = buildSessionReport([
      toolkitCall(1),
      toolkitCall(2, { sessionId: null }),
      toolkitCall(3, { sessionId: '  ' }),
      toolkitCall(4, { sessionId: undefined, requestId: undefined }),
      toolkitCall(5, { sessionId: 'sess-real' }),
    ]);
    assert.deepEqual(
      r.sessions.map((s) => [s.sessionId, s.calls]),
      [
        [UNSESSIONED, 4],
        ['sess-real', 1],
      ],
    );
    assert.match(formatSessionReport(r), /"\(unsessioned\)" groups the 4 call\(s\)/);
  });
});

describe('plans, simulations and replays', () => {
  const report = buildSessionReport(PLAN_EVENTS);
  const p = sessionOf(report, 'sess-P');

  it('counts a plan once and each step under its own tool', () => {
    assert.equal(p.calls, 1);
    assert.equal(p.steps, 2);
    assert.equal(p.collapsedWrappers, 1);
    assert.deepEqual(p.byTool, {
      agentic_execute_plan: 1,
      create_customer: 1,
      create_inventory_item: 1,
    });
    assert.deepEqual(p.plans, { count: 1, byFinalStatus: { failed: 1 } });
    // the plan's record is its outcome-free call; the steps carry outcomes
    assert.deepEqual(p.outcomes, outcomes({ executed: 1, denied: 1 }));
  });

  it('times the plan once, from its own record', () => {
    assert.deepEqual(p.latencyMs, { count: 1, p50: 40, p95: 40, max: 40 });
  });

  it('keeps the charge the wrapper event recorded', () => {
    assert.deepEqual(p.charges.byToken, [
      { tokenSymbol: 'USDC', chainId: 'base', count: 1, total: '0.50' },
    ]);
  });

  it('reads a refused step from result.result.receipt as a denial', () => {
    assert.deepEqual(classifyEvent(PLAN_EVENTS[1]), {
      outcome: 'denied',
      reasons: ['kernel.policy_denied'],
      reasonCodesUnavailable: true,
    });
    assert.equal(p.denials.reasonCodesUnavailable, 1);
    // when the codes survived, they name the denial
    const withCodes = structuredClone(PLAN_EVENTS[1]);
    withCodes.result.result.receipt.policy.reason_codes = ['policy.capability_missing:x'];
    assert.deepEqual(classifyEvent(withCodes), {
      outcome: 'denied',
      reasons: ['policy.capability_missing:x'],
    });
  });

  it('collapses the wrapper of a simulate or replay call into its record', () => {
    for (const tool of ['agentic_simulate_mutation', 'agentic_replay_mutation']) {
      const requestId = `00000000-0000-4000-8000-${tool.length.toString().padStart(12, '0')}`;
      const r = buildSessionReport([
        base({
          tool,
          source: tool,
          status: 'dry_run_success',
          requestId,
          sessionId: requestId,
          elapsedMs: 3,
          result: { simulation: true },
          notes: { simulation: true },
        }),
        base({
          tool,
          status: 'success',
          requestId: null,
          sessionId: null,
          elapsedMs: 4,
          result: mcpResult({ ok: true, preview: false, result: { requestId } }),
        }),
      ]);
      assert.equal(r.totals.calls, 1, tool);
      assert.equal(r.totals.collapsedWrappers, 1, tool);
      assert.deepEqual(r.totals.outcomes, outcomes({ dryRun: 1 }), tool);
      assert.deepEqual(r.totals.latencyMs.count, 1, tool);
    }
  });

  it('links an older wrapper through the request id at the head of its text content', () => {
    const requestId = 'b1b3a494-917c-4de6-b548-66af72638ac0';
    const r = buildSessionReport([
      base({
        tool: 'agentic_replay_mutation',
        source: 'agentic_replay_mutation',
        status: 'dry_run_success',
        requestId,
        sessionId: requestId,
      }),
      base({
        tool: 'agentic_replay_mutation',
        status: 'success',
        requestId: null,
        sessionId: null,
        result: {
          content: {
            0: { type: 'text', text: `{\n  "requestId": "${requestId}",\n  "dryRun": tru...` },
          },
        },
      }),
    ]);
    assert.equal(r.totals.calls, 1);
    assert.equal(r.totals.collapsedWrappers, 1);
  });

  it('counts a wrapper without a record (refused before it ran) as a call', () => {
    const r = buildSessionReport([
      base({
        tool: 'agentic_execute_plan',
        status: 'policy_block',
        sessionId: 's',
        error: 'Tool execution blocked by policy',
      }),
    ]);
    assert.equal(r.totals.calls, 1);
    assert.equal(r.totals.collapsedWrappers, 0);
    assert.deepEqual(r.totals.outcomes, outcomes({ denied: 1 }));
  });

  it('attributes a rollback compensation to the compensating tool', () => {
    const r = buildSessionReport([
      planEvent({
        status: 'rollback_success',
        params: { source: 'create_order', compensationTool: 'cancel_order' },
        notes: { phase: 'rollback', compensated: true, index: 0, source: 'create_order' },
      }),
    ]);
    assert.deepEqual(r.totals.byTool, { cancel_order: 1 });
    assert.equal(r.totals.steps, 1);
    assert.equal(r.totals.calls, 0);
  });
});

describe('retries that replay a receipt', () => {
  const applied = (overrides = {}) =>
    base({
      tool: 'create_refund',
      status: 'success',
      sessionId: 'sess-R',
      source: 'embedded_agent_toolkit',
      kernel: kernelBlock({ commandId: 'cmd-refund-1', receiptStatus: 'succeeded' }),
      charge: {
        charged: true,
        rule: { tool: 'create_refund', chainId: 'base', tokenSymbol: 'USDC', amount: '0.10' },
      },
      ...overrides,
    });

  it('counts the replayed receipt as replayed, not as a second write, and keeps both charges', () => {
    const r = buildSessionReport([applied(), applied()]);
    const s = sessionOf(r, 'sess-R');
    assert.equal(s.calls, 2);
    assert.deepEqual(s.outcomes, outcomes({ executed: 1, replayed: 1 }));
    assert.equal(s.charges.count, 2);
    assert.equal(s.charges.onReplayedReceipts, 1);
    assert.equal(s.charges.byToken[0].total, '0.20');
    const text = formatSessionReport(r);
    assert.match(text, /1 call\(s\) replayed an earlier kernel receipt/);
    assert.match(text, /1 charge\(s\) were taken on replayed calls/);
  });

  it('does not count a replayed refusal as a second denial', () => {
    const refused = () =>
      applied({
        status: 'error',
        charge: null,
        kernel: kernelBlock({
          commandId: 'cmd-refund-2',
          receiptStatus: 'rejected',
          errorCode: 'kernel.policy_denied',
          reasonCodes: ['policy.approval_required'],
        }),
      });
    const s = sessionOf(buildSessionReport([refused(), refused()]), 'sess-R');
    assert.deepEqual(s.outcomes, outcomes({ denied: 1, replayed: 1 }));
    assert.equal(s.denials.total, 1);
  });

  it('recognises a retry from the receipt when no kernel block was written', () => {
    const governed = () =>
      base({
        tool: 'create_customer',
        status: 'success',
        sessionId: 'sess-R',
        result: truncatedGoverned({ status: 'succeeded', command_id: 'cmd-old-1' }, { ok: true }),
      });
    const s = sessionOf(buildSessionReport([governed(), governed()]), 'sess-R');
    assert.deepEqual(s.outcomes, outcomes({ executed: 1, replayed: 1 }));
  });

  it('does not treat a preview promoted to an applied command as a retry', () => {
    const r = buildSessionReport([
      applied({
        status: 'preview',
        charge: null,
        kernel: kernelBlock({ commandId: 'cmd-refund-3', receiptStatus: 'previewed' }),
      }),
      applied({
        kernel: kernelBlock({ commandId: 'cmd-refund-3', receiptStatus: 'succeeded' }),
      }),
    ]);
    assert.deepEqual(sessionOf(r, 'sess-R').outcomes, outcomes({ previewed: 1, executed: 1 }));
  });
});

describe('executed versus previewed', () => {
  it('reports committed writes apart from previews and dry runs', () => {
    const write = { mutationManifest: { tool: 'create_order', sideEffect: 'write' } };
    const r = buildSessionReport([
      // an ungoverned MCP write: the mutation manifest marks it
      base({ tool: 'create_order', status: 'success', sessionId: 's', notes: write }),
      // a read
      base({ tool: 'get_order', status: 'success', sessionId: 's' }),
      // a preview without --apply
      base({ tool: 'create_order', status: 'preview', sessionId: 's', notes: write }),
      // a governed command the kernel previewed although the call succeeded
      base({
        tool: 'create_order',
        status: 'success',
        sessionId: 's',
        notes: write,
        kernel: kernelBlock({ receiptStatus: 'previewed' }),
      }),
      // a dry-run plan step and a blocked dry run that was only a preview
      planStep(0, 'create_order', { status: 'dry_run_success', sessionId: 's' }),
      base({
        tool: 'create_order',
        status: 'dry_run_blocked',
        sessionId: 's',
        permission: { allowed: false, preview: true },
      }),
    ]);
    const s = sessionOf(r, 's');
    assert.deepEqual(s.outcomes, outcomes({ executed: 1, ok: 1, previewed: 3, dryRun: 1 }));
    assert.deepEqual(s.byStatus, {
      success: 3,
      dry_run_blocked: 1,
      dry_run_success: 1,
      preview: 1,
    });
  });
});

describe('count keys that are Object.prototype names', () => {
  it('counts constructor, toString and __proto__ like any other key', () => {
    const r = buildSessionReport([
      base({ tool: 'constructor', status: '__proto__', sessionId: 's' }),
      base({ tool: 'constructor', status: 'toString', sessionId: 's' }),
      base({
        tool: '__proto__',
        status: 'error',
        sessionId: 's',
        result: mcpResult({ ok: false, error: { code: 'hasOwnProperty' } }),
      }),
      base({
        tool: 'toString',
        status: 'error',
        sessionId: 's',
        kernel: kernelBlock({ receiptStatus: 'rejected', reasonCodes: ['valueOf'] }),
      }),
    ]);
    const s = sessionOf(r, 's');
    assert.equal(s.calls, 4);
    assert.deepEqual(JSON.parse(JSON.stringify(s.byTool)), {
      constructor: 2,
      ['__proto__']: 1,
      toString: 1,
    });
    assert.equal(Object.getOwnPropertyDescriptor(s.byStatus, '__proto__')?.value, 1);
    assert.equal(s.byStatus.toString, 1);
    assert.equal(Object.getOwnPropertyDescriptor(s.errors.byCode, 'hasOwnProperty')?.value, 1);
    assert.equal(Object.getOwnPropertyDescriptor(s.denials.byReason, 'valueOf')?.value, 1);
    for (const counts of [s.byTool, s.byStatus, s.errors.byCode, s.denials.byReason]) {
      for (const value of Object.values(counts)) assert.equal(typeof value, 'number');
    }
  });
});

describe('classifyEvent / findKernelReceipt', () => {
  it('finds the receipt in every logged shape', () => {
    assert.equal(findKernelReceipt(EVENTS[3]).error_code, 'kernel.policy_denied');
    assert.equal(findKernelReceipt(EVENTS[4]).error_code, 'kernel.policy_denied');
    assert.equal(findKernelReceipt(PLAN_EVENTS[1]).status, 'rejected');
    assert.equal(
      findKernelReceipt({ result: { replayOutcome: { result: { receipt: receipt() } } } }).status,
      'succeeded',
    );
    assert.equal(findKernelReceipt(EVENTS[0]), null);
    assert.equal(findKernelReceipt({ result: '[truncated]' }), null);
  });

  it('groups a non-policy kernel rejection by its error_code', () => {
    const verdict = classifyEvent(
      base({
        tool: 'ship_order',
        status: 'error',
        result: {
          success: false,
          receipt: receipt({
            status: 'rejected',
            error_code: 'commerce.invalid_order_status_transition',
            policy: { allowed: true, reason_codes: [] },
          }),
        },
      }),
    );
    assert.deepEqual(verdict, {
      outcome: 'denied',
      reasons: ['commerce.invalid_order_status_transition'],
      reasonCodesUnavailable: false,
    });
  });

  it('treats a failed receipt as an error and an unknown status as other', () => {
    assert.deepEqual(
      classifyEvent({
        status: 'error',
        result: { receipt: receipt({ status: 'failed', error_code: 'kernel.storage' }) },
      }),
      { outcome: 'error', code: 'kernel.storage' },
    );
    assert.deepEqual(classifyEvent({ status: 'something_new' }), { outcome: 'other' });
  });
});

describe('the top-level kernel block', () => {
  const report = buildSessionReport(KERNEL_EVENTS);
  const k = sessionOf(report, 'sess-K');

  it('keys a refusal by the block reason codes although the receipt policy was truncated', () => {
    assert.deepEqual(k.outcomes, outcomes({ executed: 1, denied: 3, error: 1 }));
    assert.equal(k.denials.total, 3);
    assert.deepEqual(k.denials.byReason, {
      // the MCP-path denial (2 codes) and the executeTool denial, counted once
      'policy.approval_required': 2,
      'policy.max_amount_exceeded': 1,
      // no reason codes on the receipt: the block's errorCode
      'commerce.invalid_order_status_transition': 1,
    });
    assert.equal(k.denials.reasonCodesUnavailable, 0);
    assert.equal(report.totals.denials.reasonCodesUnavailable, 0);
  });

  it('prefers the block over the receipt and falls back to its errorCode', () => {
    assert.deepEqual(classifyEvent(KERNEL_EVENTS[1]), {
      outcome: 'denied',
      reasons: ['policy.approval_required', 'policy.max_amount_exceeded'],
    });
    assert.deepEqual(classifyEvent(KERNEL_EVENTS[3]), {
      outcome: 'denied',
      reasons: ['commerce.invalid_order_status_transition'],
      reasonCodesUnavailable: false,
    });
  });

  it('names the error of a failed command from the block when the result is gone', () => {
    assert.deepEqual(classifyEvent(KERNEL_EVENTS[4]), { outcome: 'error', code: 'kernel.storage' });
    assert.deepEqual(k.errors, { total: 1, byCode: { 'kernel.storage': 1 } });
  });

  it('takes the principal from the block, also on an applied command', () => {
    assert.deepEqual(k.principals, ['agent:explore']);
    assert.deepEqual(buildSessionReport([KERNEL_EVENTS[0]]).totals.principals, ['agent:explore']);
    // without the block the truncated receipt names nobody
    const { kernel: _kernel, ...withoutBlock } = KERNEL_EVENTS[0];
    assert.deepEqual(buildSessionReport([withoutBlock]).totals.principals, []);
  });

  it('counts the reason codes the writer omitted', () => {
    const cap = sessionOf(report, 'sess-cap');
    assert.equal(cap.denials.total, 1);
    assert.equal(Object.keys(cap.denials.byReason).length, 16);
    assert.equal(cap.denials.reasonCodesOmitted, 3);
    assert.equal(k.denials.reasonCodesOmitted, 0);
    assert.equal(report.totals.denials.reasonCodesOmitted, 3);
    assert.deepEqual(classifyEvent(KERNEL_EVENTS[5]).reasonCodesOmitted, 3);
  });

  it('survives a malformed block', () => {
    assert.deepEqual(
      classifyEvent({
        status: 'error',
        kernel: { receiptStatus: 'rejected', errorCode: 'kernel.policy_denied', reasonCodes: 'x' },
      }),
      { outcome: 'denied', reasons: ['kernel.policy_denied'], reasonCodesUnavailable: false },
    );
    assert.deepEqual(classifyEvent({ ...EVENTS[4], kernel: 'x' }), classifyEvent(EVENTS[4]));
    assert.deepEqual(classifyEvent({ ...EVENTS[0], kernel: null }), classifyEvent(EVENTS[0]));
  });

  it('reports old-format events exactly as before in a mixed log', () => {
    const old = buildSessionReport(EVENTS);
    const mixed = buildSessionReport([...EVENTS, ...KERNEL_EVENTS, ...MIXED_SESSION]);
    for (const key of ['sess-A', 'sess-B', UNSESSIONED]) {
      assert.deepEqual(sessionOf(mixed, key), sessionOf(old, key), key);
    }
    assert.equal(sessionOf(old, 'sess-A').denials.reasonCodesUnavailable, 1);
    assert.equal(sessionOf(old, 'sess-A').denials.reasonCodesOmitted, 0);
  });

  it('counts reasonCodesUnavailable only for the old-format refusal of a mixed session', () => {
    const mixed = buildSessionReport([...EVENTS, ...KERNEL_EVENTS, ...MIXED_SESSION]);
    const m = sessionOf(mixed, 'sess-M');
    assert.equal(m.denials.total, 2);
    assert.deepEqual(m.denials.byReason, {
      'kernel.policy_denied': 1,
      'policy.approval_required': 1,
      'policy.max_amount_exceeded': 1,
    });
    assert.equal(m.denials.reasonCodesUnavailable, 1);
    // sess-A's old refusal plus sess-M's
    assert.equal(mixed.totals.denials.reasonCodesUnavailable, 2);
  });

  it('prints the omitted-code count and no truncation caveat for new-format refusals', () => {
    const text = formatSessionReport(report);
    assert.doesNotMatch(text, /logged without reason codes/);
    assert.match(text, /3 further reason code\(s\) omitted/);
  });
});

describe('streaming reader', () => {
  it('yields events line by line and marks the line that is not JSON', async () => {
    const seen = [];
    for await (const event of readReplayLogEvents(logPath)) seen.push(event);
    assert.equal(seen.length, EVENTS.length + 1);
    assert.equal(seen.filter((event) => '_parseError' in event).length, 1);
  });

  it('builds the same report from the file, counting the malformed line', async () => {
    const fromFile = await buildSessionReportFromFile(logPath);
    assert.equal(fromFile.malformedLines, 1);
    assert.deepEqual({ ...fromFile, malformedLines: 0 }, buildSessionReport(EVENTS));
  });

  it('rejects for a log that does not exist', async () => {
    await assert.rejects(buildSessionReportFromFile(path.join(dir, 'missing.jsonl')), {
      code: 'ENOENT',
    });
  });
});

describe('formatSessionReport', () => {
  it('prints a row per session, the denial reasons and the charge caveat', () => {
    const text = formatSessionReport(buildSessionReport(EVENTS), { logPath: 'x.jsonl' });
    assert.match(text, /^Agent session report - x\.jsonl/);
    assert.match(
      text,
      /SESSION\s+CALLS\s+STEPS\s+EXEC\s+PREVIEW\s+OK\s+DENIED\s+ERRORS\s+REPLAYED\s+P50ms/,
    );
    // calls steps exec preview ok denied errors replayed p50 p95 max charges
    assert.match(text, /sess-A\s+7\s+0\s+1\s+0\s+2\s+3\s+1\s+0\s+30\s+100\s+100\s+0\.30 USDC@base/);
    assert.match(
      text,
      /TOTAL\s+11\s+0\s+1\s+1\s+3\s+4\s+2\s+0\s+10\s+100\s+100\s+0\.0001 BTC@bitcoin, 0\.30 USDC@base/,
    );
    assert.match(text, /denials: {2}.*policy\.approval_required 1/);
    assert.match(text, /1 kernel refusal\(s\) logged without reason codes/);
    assert.match(text, /2 charge amount\(s\) were logged as JSON numbers/);
    assert.match(text, /\(unsessioned\)/);
  });

  it('shows plans with their step count', () => {
    const text = formatSessionReport(buildSessionReport(PLAN_EVENTS));
    assert.match(text, /plans: {4}1 \(failed 1\), 2 step\(s\)/);
  });
});

describe('stateset-agent-report bin', () => {
  const run = async (args) => {
    try {
      const { stdout, stderr } = await exec(process.execPath, [BIN, ...args], {
        timeout: 30000,
        env: { ...process.env, NODE_NO_WARNINGS: '1' },
      });
      return { stdout, stderr, exitCode: 0 };
    } catch (error) {
      return { stdout: error.stdout || '', stderr: error.stderr || '', exitCode: error.code ?? 1 };
    }
  };

  it('prints the report object with --json and leaves the log untouched', async () => {
    const before = { bytes: readFileSync(logPath), mtimeMs: statSync(logPath).mtimeMs };
    const { stdout, exitCode } = await run(['--log', logPath, '--json']);
    assert.equal(exitCode, 0);
    const report = JSON.parse(stdout);
    assert.equal(report.logPath, logPath);
    assert.equal(report.malformedLines, 1);
    assert.equal(report.totals.calls, 11);
    const a = sessionOf(report, 'sess-A');
    assert.equal(a.charges.byToken[0].total, '0.30');
    assert.equal(typeof a.charges.byToken[0].total, 'string');
    assert.equal(a.denials.byReason['kernel.policy_denied'], 1);
    assert.deepEqual(a.latencyMs, { count: 7, p50: 30, p95: 100, max: 100 });
    assert.ok(sessionOf(report, UNSESSIONED));
    assert.ok(readFileSync(logPath).equals(before.bytes));
    assert.equal(statSync(logPath).mtimeMs, before.mtimeMs);
  });

  it('emits sessions as an array in first-seen order, whatever the ids look like', async () => {
    const orderPath = path.join(dir, 'order.jsonl');
    const ids = ['zeta', '42', '7', 'alpha'];
    writeFileSync(
      orderPath,
      `${ids
        .map((sessionId, i) =>
          JSON.stringify(
            base({
              tool: 't',
              status: 'success',
              sessionId,
              occurredAt: `2026-11-14T0${9 - i}:00:00.000Z`,
            }),
          ),
        )
        .join('\n')}\n`,
    );
    const { stdout, exitCode } = await run(['--log', orderPath, '--json']);
    assert.equal(exitCode, 0);
    const report = JSON.parse(stdout);
    assert.ok(Array.isArray(report.sessions));
    assert.deepEqual(
      report.sessions.map((s) => s.sessionId),
      ids,
    );
  });

  it('groups kernel-block refusals by reason code in a mixed log', async () => {
    const { stdout, exitCode } = await run([
      '--log',
      mixedLogPath,
      '--session',
      'sess-K',
      '--json',
    ]);
    assert.equal(exitCode, 0);
    const k = sessionOf(JSON.parse(stdout), 'sess-K');
    assert.equal(k.denials.byReason['policy.approval_required'], 2);
    assert.equal(k.denials.reasonCodesUnavailable, 0);
    assert.deepEqual(k.principals, ['agent:explore']);
  });

  it('prints a table by default and honours --session', async () => {
    const { stdout, exitCode } = await run(['--log', logPath, '--session', 'sess-B']);
    assert.equal(exitCode, 0);
    assert.match(stdout, /SESSION\s+CALLS\s+STEPS\s+EXEC\s+PREVIEW\s+OK\s+DENIED\s+ERRORS/);
    assert.match(stdout, /sess-B\s+3\s+0\s+0\s+0\s+1\s+1\s+1\s+0/);
    assert.doesNotMatch(stdout, /sess-A/);
  });

  it('exits 1 for an unreadable log or an unknown session, 2 for bad usage', async () => {
    assert.equal((await run(['--log', path.join(dir, 'missing.jsonl')])).exitCode, 1);
    const unknown = await run(['--log', logPath, '--session', 'nope']);
    assert.equal(unknown.exitCode, 1);
    assert.match(unknown.stderr, /no events for session nope/);
    assert.equal((await run([])).exitCode, 2);
    assert.equal((await run(['--bogus'])).exitCode, 2);
    for (const empty of ['', '   ']) {
      const result = await run(['--log', logPath, '--session', empty]);
      assert.equal(result.exitCode, 2, JSON.stringify(empty));
      assert.match(result.stderr, /--session needs a session id/);
    }
  });

  it('shows help and version', async () => {
    const help = await run(['--help']);
    assert.equal(help.exitCode, 0);
    assert.match(help.stdout, /stateset-agent-report --log <path>/);
    assert.match((await run(['--version'])).stdout, /^stateset-agent-report v\d+\.\d+\.\d+/);
  });
});
