import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createMcpHandler } from '@modelcontextprotocol/server';

import {
  KNOWN_TOOL_PERMISSIONS,
  READ_TOOLS_THAT_WRITE,
  deriveToolAnnotations,
} from '../../src/mcp/tool-annotations.js';
import { createStatesetV2McpServer } from '../../src/mcp/v2-server.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';

const EVERY_TOOL = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS];
const TOOL_BY_NAME = new Map(EVERY_TOOL.map((tool) => [tool.name, tool]));

const baseline = JSON.parse(
  readFileSync(new URL('../fixtures/read-tool-mutation-baseline.json', import.meta.url), 'utf8'),
);

// Tools whose effect is irreversible or removes/replaces data. Whatever their
// permission, none may ever be advertised as non-destructive.
const DESTRUCTIVE_TOOLS = [
  'delete_customer',
  'restore_database',
  'refund_payment_intent',
  'void_journal_entry',
];

// Tools that change persisted state. None may ever be advertised read-only.
const MUTATING_TOOLS = [
  'agentic_replay_mutation',
  'export_full_data',
  'agent_collect_rfq_responses',
  'resolve_payment_link',
  'x402_credit_balance',
];

// A name that says the tool removes, voids, reverses or replaces something.
const DESTRUCTIVE_NAME =
  /(^|_)(delete|remove|cancel|void|refund|reverse|revoke|purge|clear|reset|restore|overwrite|uninstall|destroy|dispose|write_off|terminate|archive|expire|deny|reject|close)(_|$)/;

describe('deriveToolAnnotations', () => {
  it('marks read tools read-only and nothing else', () => {
    assert.deepEqual(deriveToolAnnotations({ name: 'get_order', permission: 'read' }), {
      readOnlyHint: true,
    });
  });

  it('marks write tools as mutations and leaves destructiveness to the spec default', () => {
    const annotations = deriveToolAnnotations({ name: 'create_refund', permission: 'write' });
    assert.deepEqual(annotations, { readOnlyHint: false });
    assert.ok(!('destructiveHint' in annotations));
  });

  it('marks delete and admin tools destructive', () => {
    for (const permission of ['delete', 'admin']) {
      assert.deepEqual(deriveToolAnnotations({ name: 'x', permission }), {
        readOnlyHint: false,
        destructiveHint: true,
      });
    }
  });

  it('never treats an unknown or missing permission as read-only', () => {
    for (const toolDef of [
      {},
      { permission: 'unknown' },
      { permission: 'READ' },
      null,
      undefined,
    ]) {
      const annotations = deriveToolAnnotations(toolDef);
      assert.equal(annotations.readOnlyHint, false, JSON.stringify(toolDef));
      assert.equal(annotations.destructiveHint, true, JSON.stringify(toolDef));
    }
  });

  it('advertises the read tools known to write as mutations', () => {
    for (const name of READ_TOOLS_THAT_WRITE) {
      assert.deepEqual(deriveToolAnnotations({ name, permission: 'read' }), {
        readOnlyHint: false,
      });
    }
  });

  it('does not invent idempotent or open-world hints', () => {
    for (const permission of KNOWN_TOOL_PERMISSIONS) {
      const annotations = deriveToolAnnotations({ name: 'x', permission });
      assert.ok(!('idempotentHint' in annotations), permission);
      assert.ok(!('openWorldHint' in annotations), permission);
    }
  });

  it('ignores a declared readOnlyHint:true on anything not derived read-only', () => {
    for (const toolDef of [
      { name: 'w', permission: 'write' },
      { name: 'd', permission: 'delete' },
      { name: 'a', permission: 'admin' },
      { name: 'u' },
      { name: READ_TOOLS_THAT_WRITE[0], permission: 'read' },
    ]) {
      const annotations = deriveToolAnnotations({
        ...toolDef,
        annotations: { readOnlyHint: true },
      });
      assert.equal(annotations.readOnlyHint, false, JSON.stringify(toolDef));
    }
  });

  it('honours a declared readOnlyHint:false on a read tool', () => {
    assert.deepEqual(
      deriveToolAnnotations({
        name: 'r',
        permission: 'read',
        annotations: { readOnlyHint: false },
      }),
      { readOnlyHint: false },
    );
  });

  it('ignores a declared destructiveHint:false except on write tools', () => {
    for (const toolDef of [
      { name: 'd', permission: 'delete' },
      { name: 'a', permission: 'admin' },
      { name: 'u', permission: 'unknown' },
    ]) {
      const annotations = deriveToolAnnotations({
        ...toolDef,
        annotations: { destructiveHint: false },
      });
      assert.equal(annotations.destructiveHint, true, JSON.stringify(toolDef));
    }
    const readThatWrites = deriveToolAnnotations({
      name: READ_TOOLS_THAT_WRITE[0],
      permission: 'read',
      annotations: { destructiveHint: false },
    });
    assert.ok(!('destructiveHint' in readThatWrites));

    // A write tool may claim it only adds.
    assert.deepEqual(
      deriveToolAnnotations({
        name: 'create_customer',
        permission: 'write',
        annotations: { destructiveHint: false },
      }),
      { readOnlyHint: false, destructiveHint: false },
    );
  });

  it('always honours a declared destructiveHint:true', () => {
    assert.deepEqual(
      deriveToolAnnotations({
        name: 'w',
        permission: 'write',
        annotations: { destructiveHint: true },
      }),
      { readOnlyHint: false, destructiveHint: true },
    );
  });

  it('passes title, idempotentHint and openWorldHint through and drops the rest', () => {
    assert.deepEqual(
      deriveToolAnnotations({
        name: 'd',
        permission: 'delete',
        annotations: {
          title: 'Archive',
          idempotentHint: true,
          openWorldHint: false,
          readOnlyHint: 'yes',
          somethingElse: 1,
        },
      }),
      {
        readOnlyHint: false,
        destructiveHint: true,
        title: 'Archive',
        idempotentHint: true,
        openWorldHint: false,
      },
    );
    // An explicit `undefined` is not a declaration and must not erase a hint.
    assert.deepEqual(
      deriveToolAnnotations({
        name: 'r',
        permission: 'read',
        annotations: { readOnlyHint: undefined },
      }),
      { readOnlyHint: true },
    );
  });

  it('returns a fresh object and leaves the definition untouched', () => {
    const toolDef = { name: 'w', permission: 'write', annotations: { openWorldHint: false } };
    const first = deriveToolAnnotations(toolDef);
    first.readOnlyHint = true;
    assert.deepEqual(toolDef.annotations, { openWorldHint: false });
    assert.equal(deriveToolAnnotations(toolDef).readOnlyHint, false);
  });
});

describe('tool registry permissions', () => {
  it('every tool declares one of the four known permissions', () => {
    const offenders = EVERY_TOOL.filter(
      (tool) => !KNOWN_TOOL_PERMISSIONS.includes(tool.permission),
    ).map((tool) => `${tool.name}=${String(tool.permission)}`);
    assert.deepEqual(offenders, []);
  });

  it('READ_TOOLS_THAT_WRITE equals the kind:"write" entries of the read-tool mutation baseline', () => {
    const baselined = baseline.violations
      .filter((violation) => violation.kind === 'write')
      .map((violation) => violation.tool)
      .sort();
    assert.deepEqual([...READ_TOOLS_THAT_WRITE].sort(), baselined);
  });

  it('every READ_TOOLS_THAT_WRITE entry is a registered read tool', () => {
    for (const name of READ_TOOLS_THAT_WRITE) {
      assert.equal(TOOL_BY_NAME.get(name)?.permission, 'read', name);
    }
  });
});

describe('tools/list annotations on the real server', () => {
  const ENVELOPE = {
    'io.modelcontextprotocol/protocolVersion': '2026-07-28',
    'io.modelcontextprotocol/clientInfo': { name: 'annotations-test', version: '1' },
    'io.modelcontextprotocol/clientCapabilities': {},
  };

  async function listEveryTool(handler) {
    const tools = [];
    let cursor;
    do {
      const response = await handler.fetch(
        new Request('http://localhost/mcp', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Accept: 'application/json, text/event-stream',
            'Mcp-Method': 'tools/list',
            'MCP-Protocol-Version': '2026-07-28',
          },
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: 1,
            method: 'tools/list',
            params: { ...(cursor ? { cursor } : {}), _meta: ENVELOPE },
          }),
        }),
      );
      const body = await response.json();
      assert.ok(!body.error, JSON.stringify(body.error));
      tools.push(...body.result.tools);
      cursor = body.result.nextCursor;
    } while (cursor);
    return tools;
  }

  let dir;
  let stateset;
  /** @type {Map<string, any>} tools/list over the wire, by name */
  let listed;

  before(async () => {
    dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-annotations-'));
    stateset = createStatesetMcpServer({ dbPath: path.join(dir, 'store.db'), toolProfile: 'all' });
    // The server object both the stdio and HTTP entry points serve.
    const handler = createMcpHandler(() =>
      createStatesetV2McpServer({ createServer: () => stateset }),
    );
    listed = new Map((await listEveryTool(handler)).map((tool) => [tool.name, tool]));
  });

  after(async () => {
    await stateset?.close?.().catch(() => {});
    if (dir) rmSync(dir, { recursive: true, force: true });
  });

  it('the in-process (agent-sdk) tools carry the same annotations as the wire', () => {
    const adapted = stateset.getAdaptedTools();
    assert.equal(adapted.length, EVERY_TOOL.length);
    for (const tool of adapted) {
      assert.deepEqual(tool.annotations, deriveToolAnnotations(TOOL_BY_NAME.get(tool.name)));
    }
  });

  it('lists every tool with the derived annotations', () => {
    assert.equal(listed.size, EVERY_TOOL.length);
    for (const toolDef of EVERY_TOOL) {
      assert.deepEqual(
        listed.get(toolDef.name)?.annotations,
        deriveToolAnnotations(toolDef),
        toolDef.name,
      );
    }
  });

  it('never advertises a known destructive tool as non-destructive', () => {
    for (const name of DESTRUCTIVE_TOOLS) {
      const annotations = listed.get(name)?.annotations;
      assert.ok(annotations, `${name} is listed`);
      assert.equal(annotations.readOnlyHint, false, name);
      assert.notEqual(annotations.destructiveHint, false, name);
    }
  });

  it('never advertises a tool that changes state as read-only', () => {
    for (const name of MUTATING_TOOLS) {
      const annotations = listed.get(name)?.annotations;
      assert.ok(annotations, `${name} is listed`);
      assert.equal(annotations.readOnlyHint, false, name);
    }
  });

  it('never advertises a tool named for removal, reversal or replacement as non-destructive', () => {
    const offenders = [...listed.values()]
      .filter((tool) => DESTRUCTIVE_NAME.test(tool.name))
      .filter((tool) => tool.annotations?.readOnlyHint !== true)
      .filter((tool) => tool.annotations?.destructiveHint === false)
      .map((tool) => tool.name);
    assert.deepEqual(offenders, []);
  });

  it('advertises only declared write tools as non-destructive', () => {
    const offenders = [...listed.values()]
      .filter((tool) => tool.annotations?.destructiveHint === false)
      .filter((tool) => {
        const toolDef = TOOL_BY_NAME.get(tool.name);
        return toolDef.permission !== 'write' || toolDef.annotations?.destructiveHint !== false;
      })
      .map((tool) => tool.name);
    assert.deepEqual(offenders, []);
  });

  it('advertises read-only exactly for read tools not known to write', () => {
    for (const toolDef of EVERY_TOOL) {
      const expected =
        toolDef.permission === 'read' &&
        !READ_TOOLS_THAT_WRITE.includes(toolDef.name) &&
        toolDef.annotations?.readOnlyHint !== false;
      assert.equal(listed.get(toolDef.name).annotations.readOnlyHint, expected, toolDef.name);
    }
  });

  it('keeps the declared titles', () => {
    for (const toolDef of EVERY_TOOL.filter(
      (tool) => typeof tool.annotations?.title === 'string',
    )) {
      assert.equal(listed.get(toolDef.name).annotations.title, toolDef.annotations.title);
    }
  });
});
