import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createMcpHandler, InMemoryTransport } from '@modelcontextprotocol/server';
import { z } from 'zod';
import { createStatesetV2McpServer } from '../../src/mcp/v2-server.js';

const ENVELOPE = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientInfo': { name: 'schema-test', version: '1' },
  'io.modelcontextprotocol/clientCapabilities': {},
};

function fixture(inputSchema, name = 'schema_test', serverOptions = {}) {
  const calls = [];
  const handler = createMcpHandler(() =>
    createStatesetV2McpServer({
      createServer: () => ({
        ...serverOptions,
        getAdaptedTools: () => [
          {
            name,
            description: 'Test schema adaptation',
            inputSchema,
            handler: async (args) => {
              calls.push(args);
              return { content: [{ type: 'text', text: JSON.stringify(args) }] };
            },
          },
        ],
      }),
    }),
  );

  const request = async (method, params = {}) => {
    const response = await handler.fetch(
      new Request('http://localhost/mcp', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json, text/event-stream',
          'Mcp-Method': method,
          ...(params.name ? { 'Mcp-Name': params.name } : {}),
        },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          method,
          params: { ...params, _meta: ENVELOPE },
        }),
      }),
    );
    return response.json();
  };

  return {
    calls,
    list: () => request('tools/list'),
    call: (args = {}) => request('tools/call', { name, arguments: args }),
  };
}

describe('MCP v2 schema adaptation', () => {
  it('preserves custom decimal checks before calling the handler', async () => {
    const tool = fixture(
      {
        amount: z
          .string()
          .regex(/^\d+(?:\.\d+)?$/)
          .refine((value) => /[1-9]/.test(value), 'Amount must be positive'),
      },
      'positive_amount',
    );

    const refused = await tool.call({ amount: '0.00' });
    assert.equal(refused.result.isError, true);
    assert.match(refused.result.content[0].text, /Amount must be positive/);
    assert.equal(tool.calls.length, 0);

    const accepted = await tool.call({ amount: '0.000000000000000001' });
    assert.notEqual(accepted.result.isError, true);
    assert.deepEqual(tool.calls, [{ amount: '0.000000000000000001' }]);
  });

  it('applies defaults and strips unknown arguments as the original Zod schema does', async () => {
    const tool = fixture({ limit: z.number().int().positive().default(50) }, 'default_limit');
    const response = await tool.call({ unexpected: 'ignored' });
    assert.notEqual(response.result.isError, true);
    assert.deepEqual(tool.calls, [{ limit: 50 }]);
  });

  it('preserves asynchronous refinements and transformations on Zod object schemas', async () => {
    const tool = fixture(
      z
        .object({
          code: z
            .string()
            .trim()
            .transform((value) => value.toUpperCase()),
        })
        .refine(async ({ code }) => code !== 'BLOCKED', 'Code is blocked'),
      'async_schema',
    );

    const refused = await tool.call({ code: ' blocked ' });
    assert.equal(refused.result.isError, true);
    assert.equal(tool.calls.length, 0);

    await tool.call({ code: ' accepted ' });
    assert.deepEqual(tool.calls, [{ code: 'ACCEPTED' }]);
  });

  it('keeps schemas isolated when different servers use the same tool name', async () => {
    const first = fixture({ value: z.string() }, 'shared_name');
    const second = fixture({ value: z.number() }, 'shared_name');

    assert.equal((await first.list()).result.tools[0].inputSchema.properties.value.type, 'string');
    assert.equal((await second.list()).result.tools[0].inputSchema.properties.value.type, 'number');
    assert.notEqual((await second.call({ value: 42 })).result.isError, true);
    assert.deepEqual(second.calls, [{ value: 42 }]);
    assert.equal(first.calls.length, 0);
  });

  it('continues to validate literal JSON Schemas', async () => {
    const tool = fixture(
      {
        type: 'object',
        properties: { quantity: { type: 'integer', minimum: 1 } },
        required: ['quantity'],
        additionalProperties: false,
      },
      'json_schema',
    );
    assert.equal((await tool.call({ quantity: 0 })).result.isError, true);
    assert.equal(tool.calls.length, 0);
    assert.notEqual((await tool.call({ quantity: 2 })).result.isError, true);
    assert.deepEqual(tool.calls, [{ quantity: 2 }]);
  });

  it('supports no-argument tools and JSON object schemas without properties', async () => {
    for (const schema of [{}, { type: 'object', additionalProperties: false }]) {
      const tool = fixture(schema, 'no_arguments');
      assert.equal((await tool.list()).result.tools[0].inputSchema.type, 'object');
      assert.notEqual((await tool.call()).result.isError, true);
      assert.deepEqual(tool.calls, [{}]);
    }
  });
});

describe('MCP v2 lifecycle', () => {
  it('waits for service initialization before executing a tool, while discovery stays available', async () => {
    let finishInitialization;
    const servicesReady = new Promise((resolve) => {
      finishInitialization = resolve;
    });
    const tool = fixture({}, 'ready_tool', { servicesReady });
    const pendingCall = tool.call();
    try {
      assert.equal((await tool.list()).result.tools[0].name, 'ready_tool');
      await new Promise((resolve) => setImmediate(resolve));
      assert.equal(tool.calls.length, 0);
    } finally {
      finishInitialization();
      await pendingCall;
    }
    assert.deepEqual(tool.calls, [{}]);
  });

  it('cleans up on a transport disconnect without an explicit server close', async () => {
    let closes = 0;
    const server = createStatesetV2McpServer({
      createServer: () => ({
        getAdaptedTools: () => [],
        close: async () => {
          closes += 1;
        },
      }),
    });
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    await server.connect(serverTransport);
    await clientTransport.start();
    await clientTransport.close();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(closes, 1);
    await server.close();
    assert.equal(closes, 1);
  });

  it('releases the underlying server when tool registration fails', async () => {
    let closes = 0;
    const error = new Error('Cannot read tool definitions');
    assert.throws(
      () =>
        createStatesetV2McpServer({
          createServer: () => ({
            getAdaptedTools: () => {
              throw error;
            },
            close: async () => {
              closes += 1;
            },
          }),
        }),
      (actual) => actual === error,
    );
    await Promise.resolve();
    assert.equal(closes, 1);
  });

  it('releases resources when a later tool cannot be registered', async () => {
    let closes = 0;
    assert.throws(
      () =>
        createStatesetV2McpServer({
          createServer: () => ({
            getAdaptedTools: () => [
              { name: 'duplicate', inputSchema: {}, handler: async () => ({ content: [] }) },
              { name: 'duplicate', inputSchema: {}, handler: async () => ({ content: [] }) },
            ],
            close: async () => {
              closes += 1;
            },
          }),
        }),
      /already registered/,
    );
    await Promise.resolve();
    assert.equal(closes, 1);
  });

  it('waits for resource cleanup and runs it once across repeated closes', async () => {
    let finishCleanup;
    let closes = 0;
    const pendingCleanup = new Promise((resolve) => {
      finishCleanup = resolve;
    });
    const server = createStatesetV2McpServer({
      createServer: () => ({
        getAdaptedTools: () => [],
        close: () => {
          closes += 1;
          return pendingCleanup;
        },
      }),
    });
    let finished = false;
    const closing = server.close().then(() => {
      finished = true;
    });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(closes, 1);
    assert.equal(finished, false);
    finishCleanup();
    await closing;
    await server.close();
    assert.equal(finished, true);
    assert.equal(closes, 1);
  });

  it('closes the transport even when resource cleanup fails', async () => {
    const error = new Error('Resource cleanup failed');
    const server = createStatesetV2McpServer({
      createServer: () => ({
        getAdaptedTools: () => [],
        dispose: () => {
          throw error;
        },
      }),
    });
    let transportClosed = false;
    server.server.close = async () => {
      transportClosed = true;
    };
    await assert.rejects(server.close(), (actual) => actual === error);
    assert.equal(transportClosed, true);
  });
});
