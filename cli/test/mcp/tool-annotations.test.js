import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createMcpHandler } from '@modelcontextprotocol/server';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { z } from 'zod';

import { KNOWN_TOOL_PERMISSIONS, deriveToolAnnotations } from '../../src/mcp/tool-annotations.js';
import {
  createAnnotatedSdkMcpServer,
  sdkForwardsToolAnnotations,
} from '../../src/mcp/sdk-mcp-server.js';
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
  'agentic_unsubscribe_events',
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
    // A read tool's hints say nothing about destructiveness.
    assert.deepEqual(
      deriveToolAnnotations({
        name: 'r',
        permission: 'read',
        annotations: { destructiveHint: false },
      }),
      { readOnlyHint: true },
    );

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

  // Every read tool is advertised read-only, so none may be excused for
  // writing rows. The baseline may excuse only first-use `init` work (an
  // empty table, index, database or directory).
  it('the read-tool mutation baseline excuses no read tool that writes rows', () => {
    const dataWrites = baseline.violations
      .filter((violation) => violation.kind !== 'init')
      .map((violation) => `${violation.tool} (${violation.kind})`);
    assert.deepEqual(
      dataWrites,
      [],
      'tools/list would advertise these read-only: declare them write instead of baselining them',
    );
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

  it('advertises read-only exactly for read tools', () => {
    for (const toolDef of EVERY_TOOL) {
      const expected = toolDef.permission === 'read' && toolDef.annotations?.readOnlyHint !== false;
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

/** Connect a real MCP client to an McpServer over an in-memory transport. */
async function connectClient(mcpServer) {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  await mcpServer.connect(serverTransport);
  const client = new Client({ name: 'annotations-test', version: '1' });
  await client.connect(clientTransport);
  return client;
}

/** Every page of tools/list, by name. */
async function listToolsByName(client) {
  const tools = [];
  let cursor;
  do {
    const page = await client.listTools(cursor ? { cursor } : {});
    tools.push(...page.tools);
    cursor = page.nextCursor;
  } while (cursor);
  return new Map(tools.map((tool) => [tool.name, tool]));
}

// agent-sdk before 0.2.27, including the 0.1.72 that package-lock.json pins:
// its tool() and createSdkMcpServer(), copied verbatim, on the MCP SDK's
// McpServer. The four-argument server.tool() has nowhere to put annotations.
const LEGACY_AGENT_SDK = Object.freeze({
  sdkTool: (name, description, inputSchema, handler) => ({
    name,
    description,
    inputSchema,
    handler,
  }),
  createSdkMcpServer: (options) => {
    const server = new McpServer(
      { name: options.name, version: options.version ?? '1.0.0' },
      { capabilities: { tools: options.tools ? {} : undefined } },
    );
    options.tools?.forEach((toolDef) => {
      server.tool(toolDef.name, toolDef.description, toolDef.inputSchema, toolDef.handler);
    });
    return { type: 'sdk', name: options.name, instance: server };
  },
});

// agent-sdk 0.2.27 and later: tool() keeps `extras.annotations`.
const forwardingSdkTool = (name, description, inputSchema, handler, extras) => ({
  name,
  description,
  inputSchema,
  handler,
  annotations: extras?.annotations,
});

/** Drop `annotations` from listed tools, to compare everything else. */
const withoutAnnotations = (listed) =>
  new Map([...listed].map(([name, { annotations: _annotations, ...rest }]) => [name, rest]));

describe('tools/list annotations on the in-process agent-sdk server', () => {
  let dir;
  let stateset;
  let client;
  /** @type {Map<string, any>} tools/list from the agent-sdk server instance */
  let listed;

  before(async () => {
    dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-sdk-annotations-'));
    stateset = createStatesetMcpServer({ dbPath: path.join(dir, 'store.db'), toolProfile: 'all' });
    // The McpServer the agent-sdk built: what query() talks to in-process.
    client = await connectClient(stateset.instance);
    listed = await listToolsByName(client);
  });

  after(async () => {
    await client?.close().catch(() => {});
    await stateset?.close?.().catch(() => {});
    if (dir) rmSync(dir, { recursive: true, force: true });
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

  it('advertises read, delete and write tools with the hints their permission allows', () => {
    assert.equal(TOOL_BY_NAME.get('get_order').permission, 'read');
    assert.deepEqual(listed.get('get_order').annotations, { readOnlyHint: true });

    for (const name of ['cancel_order', 'delete_cart']) {
      assert.equal(TOOL_BY_NAME.get(name).permission, 'delete', name);
      assert.deepEqual(
        listed.get(name).annotations,
        { readOnlyHint: false, destructiveHint: true },
        name,
      );
    }

    for (const name of ['create_customer', 'delete_customer', 'refund_payment_intent']) {
      assert.equal(TOOL_BY_NAME.get(name).permission, 'write', name);
      const annotations = listed.get(name).annotations;
      assert.equal(annotations.readOnlyHint, false, name);
      assert.ok(!('destructiveHint' in annotations), `${name} must not claim destructiveHint`);
    }
  });

  it('calls the wrapped tool handler', async () => {
    const result = await client.callTool({ name: 'list_customers', arguments: {} });
    assert.notEqual(result.isError, true, JSON.stringify(result));
    assert.equal(result.content[0].type, 'text');
    assert.ok(JSON.parse(result.content[0].text));
  });

  it('registers every tool as a legacy agent-sdk would, plus its annotations', async () => {
    const tools = stateset.getAdaptedTools();
    const legacy = LEGACY_AGENT_SDK.createSdkMcpServer({ name: 'parity', tools });
    const annotated = createAnnotatedSdkMcpServer({ name: 'parity', tools }, LEGACY_AGENT_SDK);
    const legacyClient = await connectClient(legacy.instance);
    const annotatedClient = await connectClient(annotated.instance);
    try {
      const legacyListed = await listToolsByName(legacyClient);
      const annotatedListed = await listToolsByName(annotatedClient);

      // The bug: a legacy agent-sdk lists no hints at all.
      assert.deepEqual(
        [...legacyListed.values()].filter((tool) => tool.annotations),
        [],
      );
      // Same names, descriptions and input schemas; only annotations added.
      assert.deepEqual(withoutAnnotations(annotatedListed), withoutAnnotations(legacyListed));
      for (const tool of tools) {
        assert.deepEqual(annotatedListed.get(tool.name).annotations, tool.annotations, tool.name);
      }
    } finally {
      await legacyClient.close();
      await annotatedClient.close();
    }
  });
});

describe('createAnnotatedSdkMcpServer', () => {
  const sampleTools = () => [
    {
      name: 'get_thing',
      description: 'Read a thing',
      inputSchema: { id: z.string().describe('Thing id') },
      handler: async (args) => ({ content: [{ type: 'text', text: `got ${args.id}` }] }),
      annotations: { readOnlyHint: true },
    },
    {
      name: 'ping',
      description: 'No arguments',
      inputSchema: {},
      handler: async (args) => ({ content: [{ type: 'text', text: JSON.stringify(args) }] }),
      annotations: { readOnlyHint: false, destructiveHint: true },
    },
  ];

  it('tells an agent-sdk that forwards annotations from one that drops them', () => {
    assert.equal(sdkForwardsToolAnnotations(LEGACY_AGENT_SDK.sdkTool), false);
    assert.equal(sdkForwardsToolAnnotations(forwardingSdkTool), true);
    assert.equal(
      sdkForwardsToolAnnotations(() => {
        throw new Error('unexpected arguments');
      }),
      false,
    );
  });

  it('on an agent-sdk that drops annotations, lists them and keeps the handlers', async () => {
    const tools = sampleTools();
    const server = createAnnotatedSdkMcpServer(
      { name: 'sample', version: '2.0.0', tools },
      LEGACY_AGENT_SDK,
    );
    assert.equal(server.type, 'sdk');
    assert.equal(server.name, 'sample');
    const client = await connectClient(server.instance);
    try {
      assert.deepEqual(client.getServerVersion(), { name: 'sample', version: '2.0.0' });
      const listed = await listToolsByName(client);
      assert.deepEqual(listed.get('get_thing').annotations, { readOnlyHint: true });
      assert.deepEqual(listed.get('ping').annotations, {
        readOnlyHint: false,
        destructiveHint: true,
      });
      assert.equal(listed.get('get_thing').inputSchema.properties.id.description, 'Thing id');
      assert.deepEqual(listed.get('get_thing').inputSchema.required, ['id']);

      const got = await client.callTool({ name: 'get_thing', arguments: { id: 'a1' } });
      assert.deepEqual(got.content, [{ type: 'text', text: 'got a1' }]);
      const pinged = await client.callTool({ name: 'ping', arguments: {} });
      assert.deepEqual(pinged.content, [{ type: 'text', text: '{}' }]);
      const invalid = await client.callTool({ name: 'get_thing', arguments: {} });
      assert.equal(invalid.isError, true);
    } finally {
      await client.close();
    }
  });

  it('hands the tools to an agent-sdk that forwards annotations untouched', () => {
    const calls = [];
    const createSdkMcpServer = (options) => {
      calls.push(options);
      return { type: 'sdk', name: options.name, instance: {} };
    };
    const options = { name: 'sample', version: '2.0.0', tools: sampleTools() };
    createAnnotatedSdkMcpServer(options, { createSdkMcpServer, sdkTool: forwardingSdkTool });
    assert.equal(calls.length, 1);
    assert.equal(calls[0], options);
  });

  it('leaves a tool list without annotations to the agent-sdk', () => {
    const calls = [];
    const createSdkMcpServer = (options) => {
      calls.push(options);
      return { type: 'sdk', name: options.name, instance: {} };
    };
    const tools = sampleTools().map(({ annotations: _annotations, ...rest }) => rest);
    const options = { name: 'sample', tools };
    createAnnotatedSdkMcpServer(options, { ...LEGACY_AGENT_SDK, createSdkMcpServer });
    assert.equal(calls.length, 1);
    assert.equal(calls[0], options);
  });
});
