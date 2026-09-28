/**
 * The tool result contract, enforced on the real MCP server.
 *
 * Agents act on what a tool returns. Every call -- whatever the tool, whether
 * it succeeds, refuses, is previewed or is rejected by the kernel -- must come
 * back as one shape: `{ ok, preview, result?, error?: { code, message,
 * retryable, hint? }, notice? }` (src/mcp/tool-result-contract.js).
 *
 *   - The whole catalog in preview mode (no --apply): every outcome is a valid
 *     contract, and every write is an ok preview -- never an error.
 *     (The apply-mode sweep is checked in mcp-tool-smoke.test.js.)
 *   - A kernel receipt that was rejected is `ok: false` with the receipt's own
 *     code, on `executeTool` and on the MCP `CallToolResult` (`isError`).
 *   - Schema-invalid input is `INVALID_INPUT`.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { runSmoke } from '../../scripts/mcp-tool-smoke.mjs';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../src/kernel-tool-execution.js';
import {
  ToolErrorCode,
  ToolNoticeCode,
  toToolResultContract,
  validateToolResultContract,
} from '../../src/mcp/tool-result-contract.js';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';

const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-contract-'));
const capabilities = [...new Set(Object.values(KERNEL_CAPABILITY_BY_TOOL))];
const kernel = {
  strict: false,
  storeId: 'store:contract',
  principal: {
    id: 'agent:contract',
    kind: 'agent',
    tenantId: 'tenant:contract',
    delegatedBy: 'user:contract',
    capabilities,
  },
  policy: {
    version: 'contract-v1',
    commands: Object.fromEntries(
      capabilities.map((capability) => [
        capability,
        {
          required_capabilities: [capability],
          requires_approval: false,
          requires_tenant: true,
          requires_store: true,
          allowed_tenant_ids: ['tenant:contract'],
          allowed_store_ids: ['store:contract'],
          requires_agent_delegation: true,
          requires_signed_authority: false,
        },
      ]),
    ),
    trusted_authority_keys: {},
  },
};

let applyServer;
let previewServer;

before(() => {
  applyServer = createStatesetMcpServer({
    dbPath: path.join(dir, 'apply.db'),
    allowApply: true,
    kernel,
  });
  previewServer = createStatesetMcpServer({ dbPath: path.join(dir, 'preview.db') });
});

after(async () => {
  for (const server of [applyServer, previewServer]) {
    try {
      await server?.close?.();
    } catch {
      // never connected to a transport
    }
  }
  rmSync(dir, { recursive: true, force: true });
});

const adapted = (server, name) => {
  const tool = server.getAdaptedTools().find((t) => t.name === name);
  assert.ok(tool, `${name} is exposed`);
  return tool;
};

describe('the tool result contract on the real server', () => {
  it('reports a rejected kernel receipt as ok:false with the receipt code', async () => {
    const customer = await applyServer.executeTool('create_customer', {
      email: 'grace@example.com',
      firstName: 'Grace',
      lastName: 'Hopper',
    });
    assert.equal(customer.ok, true, JSON.stringify(customer.failure));
    const order = await applyServer.executeTool('create_order', {
      customerId: customer.result.customer.id,
      items: [{ sku: 'W-1', name: 'Widget', quantity: 1, unitPrice: 5 }],
    });
    assert.equal(order.ok, true, JSON.stringify(order.failure));
    assert.equal(typeof order.result.order.totalAmountExact, 'string');

    // A pending order cannot ship: the kernel rejects the command.
    const shipped = await applyServer.executeTool('ship_order', {
      orderId: order.result.order.id,
      trackingNumber: '1Z',
    });
    assert.equal(shipped.success, false);
    assert.equal(shipped.ok, false);
    assert.equal(shipped.preview, false);
    const receipt = shipped.result.receipt;
    assert.equal(receipt.status, 'rejected');
    assert.equal(shipped.failure.code, receipt.error_code);
    assert.equal(shipped.failure.message, receipt.error_message);
    assert.equal(shipped.failure.retryable, false);
    assert.deepEqual(validateToolResultContract(toToolResultContract(shipped)), []);

    // The same refusal through the MCP tool both transports serve.
    const response = await adapted(applyServer, 'ship_order').handler(
      { orderId: order.result.order.id, trackingNumber: '1Z' },
      {},
    );
    assert.equal(response.isError, true);
    assert.deepEqual(validateToolResultContract(response.structuredContent), []);
    assert.equal(response.structuredContent.ok, false);
    assert.equal(response.structuredContent.error.code, receipt.error_code);
    // Text-only clients still see the tool's own JSON.
    assert.equal(JSON.parse(response.content[0].text).receipt.status, 'rejected');
  });

  it('reports schema-invalid input as INVALID_INPUT', async () => {
    const out = await applyServer.executeTool('get_order', {});
    assert.equal(out.ok, false);
    assert.equal(out.failure.code, ToolErrorCode.INVALID_INPUT);
    assert.match(out.failure.hint, /identifier/);
    assert.deepEqual(validateToolResultContract(toToolResultContract(out)), []);
  });

  it('reports an unknown tool as UNKNOWN_TOOL', async () => {
    const out = await applyServer.executeTool('no_such_tool', {});
    assert.equal(out.ok, false);
    assert.equal(out.failure.code, ToolErrorCode.UNKNOWN_TOOL);
  });

  it('keeps a binding error code from a thrown engine error', async () => {
    const out = await applyServer.executeTool('get_order', { identifier: 'not-a-uuid' });
    assert.equal(out.ok, false);
    assert.equal(out.failure.code, 'VALIDATION');
  });

  it('marks a write without --apply as an ok preview, not an error', async () => {
    const params = { email: 'ada@example.com', firstName: 'Ada', lastName: 'L' };
    const out = await previewServer.executeTool('create_customer', params);
    assert.equal(out.ok, true);
    assert.equal(out.preview, true);
    assert.equal(out.success, false, 'legacy success still means "executed"');
    assert.equal(out.notice.code, ToolNoticeCode.APPLY_REQUIRED);

    const response = await adapted(previewServer, 'create_customer').handler(params, {});
    assert.notEqual(response.isError, true);
    assert.equal(response.structuredContent.ok, true);
    assert.equal(response.structuredContent.preview, true);
    assert.equal(response.structuredContent.notice.code, ToolNoticeCode.APPLY_REQUIRED);
  });
});

describe('every tool in preview mode answers with a valid contract', async () => {
  const rows = await runSmoke({ allowApply: false, timeoutMs: 30_000 });
  const permissionByTool = new Map(ALL_DOMAIN_TOOLS.map((t) => [t.name, t.permission]));

  it('exercises the whole catalog', () => {
    assert.equal(rows.length, ALL_DOMAIN_TOOLS.length);
  });

  it('never throws past the contract and always returns a valid outcome', () => {
    const bad = rows
      .filter((row) => row.kind !== 'timeout')
      .map((row) => ({
        tool: row.tool,
        problems: row.contract
          ? validateToolResultContract(row.contract)
          : [`threw: ${row.message}`],
      }))
      .filter((row) => row.problems.length > 0);
    assert.deepEqual(bad, []);
  });

  it('previews every write with valid input instead of refusing it', () => {
    // Input is validated before the --apply gate, so a synthetic input the
    // schema's refinements reject is INVALID_INPUT -- correctly, not a preview.
    const notPreviewed = rows
      .filter((row) => row.contract && permissionByTool.get(row.tool) !== 'read')
      .filter((row) => row.code !== ToolErrorCode.INVALID_INPUT)
      .filter((row) => !(row.contract.ok && row.contract.preview))
      .map((row) => `${row.tool}: ${row.code}: ${row.message}`);
    assert.deepEqual(notPreviewed, []);
  });
});
