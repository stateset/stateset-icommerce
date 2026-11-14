// stateset-agent-report over replay logs written by a real server.
//
// test/mcp/session-report.test.js pins the report against trimmed event
// fixtures. These tests build the server the way the bins do
// (createStatesetMcpServer over a temp SQLite file), drive tools through the
// MCP handler (`getAdaptedTools()[i].handler(args, extra)`, the path an MCP
// client reaches) and through `executeTool` (the embedded toolkit path), and
// report on the agentic-tool-calls.jsonl the server wrote, so the report is
// checked against the writers as they are.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { getCommerce } from '../../src/database.js';
import {
  buildSessionReport,
  classifyEvent,
  findKernelReceipt,
} from '../../src/mcp/session-report.js';

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
const sessionOf = (report, id) => report.sessions.find((s) => s.sessionId === id);

/** A server over a fresh store; its replay log lands in `<dir>/state`. */
async function realServer(t, options = {}) {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'session-report-real-'));
  const dbPath = path.join(dir, 'store.db');
  const policyStorePath = path.join(dir, 'state');
  const server = createStatesetMcpServer({ dbPath, policyStorePath, ...options });
  await server.servicesReady;
  t.after(async () => {
    server.dispose?.();
    await server.close?.().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  });
  const logPath = path.join(policyStorePath, 'agentic-tool-calls.jsonl');
  return {
    server,
    dir,
    dbPath,
    logPath,
    /** Call a tool through its MCP handler, as an MCP client would. */
    mcp: (name, args, extra = {}) =>
      server
        .getAdaptedTools()
        .find((tool) => tool.name === name)
        .handler(args, extra),
    events: () =>
      readFileSync(logPath, 'utf8')
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line)),
  };
}

describe('tool-level dry runs (close_month, import_full_data)', () => {
  it('counts a successful dryRun call as a dry run, not a committed write', async (t) => {
    const { server, dir, dbPath, mcp, events } = await realServer(t, { allowApply: true });
    const ok = (r, what) => assert.equal(r.status, 'success', `${what}: ${r.error}`);

    const accounts = await server.executeTool('initialize_chart_of_accounts', {});
    ok(accounts, 'chart of accounts');
    const created = await server.executeTool('create_gl_period', {
      periodName: 'FY',
      fiscalYear: 2026,
      periodNumber: 1,
      startDate: '2020-01-01',
      endDate: '2030-12-31',
    });
    ok(created, 'period');
    const periodId = created.result.period.id;
    ok(await server.executeTool('open_gl_period', { periodId }), 'open period');
    // Newer engine builds refuse a month-end close until automatic ledger
    // posting is configured; older ones have no such setting.
    const gl = getCommerce(dbPath).generalLedger;
    if (typeof gl.setAutoPostingConfig === 'function') {
      const id = (n) => accounts.result.accounts.find((a) => a.accountNumber === n).id;
      await gl.setAutoPostingConfig({
        configName: 'default',
        cashAccountId: id('1010'),
        accountsReceivableAccountId: id('1100'),
        inventoryAccountId: id('1200'),
        accountsPayableAccountId: id('2010'),
        salesRevenueAccountId: id('4010'),
        cogsAccountId: id('5010'),
        autoPostDepreciation: true,
        autoPostRevenueRecognition: true,
      });
    }
    const exportPath = path.join(dir, 'export.json');
    ok(await server.executeTool('export_full_data', { exportPath }), 'export');

    const before = events().length;
    // MCP: logged `success` with a mutation manifest, as a write would be.
    const close = await mcp('close_month', { periodId, dryRun: true }, { sessionId: 'sess-dry' });
    assert.equal(close.structuredContent.ok, true, JSON.stringify(close.structuredContent));
    assert.equal(close.structuredContent.result.report.dryRun, true);
    const imported = await mcp(
      'import_full_data',
      { importPath: exportPath, dryRun: true },
      { sessionId: 'sess-dry' },
    );
    assert.equal(imported.structuredContent.ok, true, JSON.stringify(imported.structuredContent));
    // The embedded toolkit path, and a plan step.
    ok(
      await server.executeTool(
        'import_full_data',
        { importPath: exportPath, dryRun: true },
        { sessionId: 'sess-dry' },
      ),
      'toolkit import dry run',
    );
    const plan = await server.executeTool(
      'agentic_execute_plan',
      {
        steps: [{ tool: 'close_month', params: { periodId, dryRun: true } }],
        dryRun: false,
        sessionId: 'sess-dry',
      },
      { sessionId: 'sess-dry' },
    );
    assert.equal(plan.result.finalStatus, 'success', JSON.stringify(plan.result.steps));
    // An applied write stays a committed write.
    const applied = await mcp(
      'import_full_data',
      { importPath: exportPath },
      { sessionId: 'sess-dry' },
    );
    assert.equal(applied.structuredContent.ok, true, JSON.stringify(applied.structuredContent));

    const logged = events().slice(before);
    const mcpClose = logged.find((e) => e.source === 'mcp_server' && e.tool === 'close_month');
    assert.equal(mcpClose.status, 'success');
    assert.equal(typeof mcpClose.notes.mutationManifest, 'object');
    assert.notEqual(mcpClose.notes.mutationManifest, null);

    const s = sessionOf(buildSessionReport(logged), 'sess-dry');
    // close_month (MCP), import (MCP), import (toolkit), the plan's
    // close_month step; the applied import is the one committed write.
    assert.deepEqual(s.outcomes, outcomes({ dryRun: 4, executed: 1 }));
    assert.equal(s.calls, 5);
    assert.equal(s.steps, 1);
  });
});

/**
 * A trusted kernel that governs customer and inventory creation; the
 * principal holds only the inventory capability, so the kernel refuses a
 * customers.create command (policy.capability_missing).
 */
const commandRule = (capability) => ({
  required_capabilities: [capability],
  requires_approval: false,
  requires_tenant: true,
  requires_store: true,
  requires_agent_delegation: true,
  requires_signed_authority: false,
});
const KERNEL = {
  strict: false,
  storeId: 'store:report',
  principal: {
    id: 'agent:report',
    kind: 'agent',
    tenantId: 'tenant:report',
    delegatedBy: 'user:report',
    capabilities: ['inventory.item.create'],
  },
  policy: {
    version: 'report-v1',
    commands: {
      'customers.create': commandRule('customers.create'),
      'inventory.item.create': commandRule('inventory.item.create'),
    },
    trusted_authority_keys: {},
  },
};
const SIMULATE = 'agentic_simulate_mutation';
const CUSTOMER = { email: 'ada@example.com', firstName: 'Ada', lastName: 'L' };

describe('simulation refusals', () => {
  it('reports a kernel refusal inside a simulation as a denial, not a preview', async (t) => {
    // No --apply: a simulated governed write runs as a kernel preview.
    const { server, mcp, events } = await realServer(t, { kernel: KERNEL });
    const refusedMcp = await mcp(
      SIMULATE,
      { tool: 'create_customer', params: CUSTOMER, sessionId: 'sess-sim' },
      { sessionId: 'sess-sim' },
    );
    assert.equal(refusedMcp.structuredContent.ok, true);
    const refused = await server.executeTool(
      SIMULATE,
      { tool: 'create_customer', params: CUSTOMER, sessionId: 'sess-sim' },
      { sessionId: 'sess-sim' },
    );
    assert.equal(refused.result.outcome.result.receipt.status, 'rejected');
    const previewed = await server.executeTool(
      SIMULATE,
      {
        tool: 'create_inventory_item',
        params: { sku: 'SKU-1', name: 'Widget', initialQuantity: '5' },
        sessionId: 'sess-sim',
      },
      { sessionId: 'sess-sim' },
    );
    assert.equal(previewed.result.outcome.result.receipt.status, 'previewed');

    const records = events().filter((e) => e.source === SIMULATE);
    assert.equal(records.length, 3);
    // The writer logged the refusals as blocked previews; only the receipt
    // at result.simulation.result.receipt says the kernel refused them.
    for (const record of records.slice(0, 2)) {
      assert.equal(record.status, 'dry_run_blocked');
      assert.equal(record.permission.preview, true);
      assert.equal(findKernelReceipt(record).status, 'rejected');
      assert.deepEqual(classifyEvent(record), {
        outcome: 'denied',
        reasons: ['kernel.policy_denied'],
        // the replay sanitizer cut the receipt's policy decision
        reasonCodesUnavailable: true,
      });
    }
    assert.equal(findKernelReceipt(records[2]).status, 'previewed');

    const s = sessionOf(buildSessionReport(events()), 'sess-sim');
    assert.equal(s.calls, 3);
    assert.deepEqual(s.outcomes, outcomes({ denied: 2, previewed: 1 }));
    assert.deepEqual(s.denials.byReason, { 'kernel.policy_denied': 2 });
    assert.equal(s.denials.reasonCodesUnavailable, 2);
  });
});
