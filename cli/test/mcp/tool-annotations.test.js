import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createMcpHandler } from '@modelcontextprotocol/server';

import { KNOWN_TOOL_PERMISSIONS, deriveToolAnnotations } from '../../src/mcp/tool-annotations.js';
import { createStatesetV2McpServer } from '../../src/mcp/v2-server.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';

const EVERY_TOOL = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS];

describe('deriveToolAnnotations', () => {
  it('marks read tools read-only and nothing else', () => {
    assert.deepEqual(deriveToolAnnotations({ permission: 'read' }), { readOnlyHint: true });
  });

  it('marks write tools as non-destructive mutations', () => {
    assert.deepEqual(deriveToolAnnotations({ permission: 'write' }), {
      readOnlyHint: false,
      destructiveHint: false,
    });
  });

  it('marks delete tools destructive', () => {
    assert.deepEqual(deriveToolAnnotations({ permission: 'delete' }), {
      readOnlyHint: false,
      destructiveHint: true,
    });
  });

  it('treats admin tools conservatively as destructive', () => {
    assert.deepEqual(deriveToolAnnotations({ permission: 'admin' }), {
      readOnlyHint: false,
      destructiveHint: true,
    });
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

  it('does not invent idempotent or open-world hints', () => {
    for (const permission of KNOWN_TOOL_PERMISSIONS) {
      const annotations = deriveToolAnnotations({ permission });
      assert.ok(!('idempotentHint' in annotations), permission);
      assert.ok(!('openWorldHint' in annotations), permission);
    }
  });

  it('lets explicit annotations override derived values field by field', () => {
    assert.deepEqual(
      deriveToolAnnotations({
        permission: 'delete',
        annotations: { title: 'Archive', destructiveHint: false, idempotentHint: true },
      }),
      { readOnlyHint: false, destructiveHint: false, title: 'Archive', idempotentHint: true },
    );
    // An explicit `undefined` is not a declaration and must not erase a hint.
    assert.deepEqual(
      deriveToolAnnotations({ permission: 'read', annotations: { readOnlyHint: undefined } }),
      { readOnlyHint: true },
    );
  });

  it('returns a fresh object and leaves the definition untouched', () => {
    const toolDef = { permission: 'write', annotations: { openWorldHint: false } };
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

  it(
    'advertises derived and declared annotations for the whole catalog',
    { timeout: 150_000 },
    async (t) => {
      const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-annotations-'));
      try {
        const dbPath = path.join(dir, 'store.db');
        const stateset = createStatesetMcpServer({ dbPath, toolProfile: 'all' });

        // In-process (agent-sdk) surface: the adapted tools carry annotations.
        const adapted = stateset.getAdaptedTools();
        assert.equal(adapted.length, EVERY_TOOL.length);
        for (const tool of adapted) {
          assert.equal(typeof tool.annotations?.readOnlyHint, 'boolean', tool.name);
        }

        // Wire surface: the server object both stdio and HTTP entry points serve.
        const handler = createMcpHandler(() =>
          createStatesetV2McpServer({ createServer: () => stateset }),
        );
        const listed = await listEveryTool(handler);
        const byName = new Map(listed.map((tool) => [tool.name, tool]));
        assert.equal(byName.size, EVERY_TOOL.length);

        const counts = {};
        for (const toolDef of EVERY_TOOL) {
          const annotations = byName.get(toolDef.name)?.annotations;
          assert.equal(typeof annotations?.readOnlyHint, 'boolean', toolDef.name);
          if (!toolDef.annotations) {
            assert.equal(annotations.readOnlyHint, toolDef.permission === 'read', toolDef.name);
          }
          if (toolDef.permission === 'delete') {
            assert.equal(annotations.destructiveHint, true, toolDef.name);
          }
          const key = `${toolDef.permission}:readOnly=${annotations.readOnlyHint},destructive=${annotations.destructiveHint}`;
          counts[key] = (counts[key] || 0) + 1;
        }

        const declared = EVERY_TOOL.filter((tool) => tool.annotations);
        assert.equal(declared.length, 5);
        for (const toolDef of declared) {
          const annotations = byName.get(toolDef.name).annotations;
          for (const [field, value] of Object.entries(toolDef.annotations)) {
            assert.deepEqual(annotations[field], value, `${toolDef.name}.${field}`);
          }
        }
        t.diagnostic(`annotation-counts ${JSON.stringify(counts)}`);
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    },
  );
});
