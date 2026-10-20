import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createMcpHandler } from '@modelcontextprotocol/server';
import { A2AStore } from '../../src/a2a/store.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { createStatesetV2McpServer } from '../../src/mcp/v2-server.js';

describe('MCP server resource ownership', () => {
  it('releases the connection of each stateless HTTP request', async (t) => {
    const dir = await mkdtemp(join(tmpdir(), 'mcp-request-close-'));
    const closedStores = [];
    const closeStore = A2AStore.prototype.close;
    t.mock.method(A2AStore.prototype, 'close', function () {
      closedStores.push(this);
      return closeStore.call(this);
    });
    const servers = [];
    const handler = createMcpHandler(() =>
      createStatesetV2McpServer({
        createServer: (options) => {
          const server = createStatesetMcpServer(options);
          servers.push(server);
          return server;
        },
        commerce: {},
        dbPath: join(dir, 'store.db'),
      }),
    );
    try {
      for (let id = 1; id <= 3; id += 1) {
        const response = await handler.fetch(
          new Request('http://localhost/mcp', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Accept: 'application/json, text/event-stream',
              'Mcp-Method': 'tools/call',
              'Mcp-Name': 'a2a_list_failed_notifications',
            },
            body: JSON.stringify({
              jsonrpc: '2.0',
              id,
              method: 'tools/call',
              params: {
                name: 'a2a_list_failed_notifications',
                arguments: {},
                _meta: {
                  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
                  'io.modelcontextprotocol/clientInfo': { name: 'lifecycle-test', version: '1' },
                  'io.modelcontextprotocol/clientCapabilities': {},
                },
              },
            }),
          }),
        );
        const result = await response.json();
        assert.notEqual(result.result.isError, true, JSON.stringify(result));
        await new Promise((resolve) => setImmediate(resolve));
        assert.equal(closedStores.length, id);
        assert.ok(closedStores.every((store) => store.db === null));
      }
      assert.equal(new Set(closedStores).size, 3);
    } finally {
      await handler.close();
      await Promise.all(servers.map((server) => server.close()));
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('closes its A2A database without closing the shared commerce engine', async (t) => {
    const dir = await mkdtemp(join(tmpdir(), 'mcp-lifecycle-'));
    const closedStores = [];
    const closeStore = A2AStore.prototype.close;
    t.mock.method(A2AStore.prototype, 'close', function () {
      closedStores.push(this);
      return closeStore.call(this);
    });
    let commerceCloses = 0;
    const commerce = {
      customers: { list: async () => [], count: async () => 0 },
      close: () => {
        commerceCloses += 1;
      },
    };
    const server = createStatesetMcpServer({ commerce, dbPath: join(dir, 'store.db') });
    try {
      await server.servicesReady;
      const response = await server.executeTool('a2a_list_failed_notifications');
      assert.equal(response.success, true, JSON.stringify(response));
      await server.close();
      await server.close();
      assert.equal(closedStores.length, 1);
      assert.equal(closedStores[0].db, null);
      assert.equal(commerceCloses, 0);
      assert.equal((await commerce.customers.list()).length, 0);
    } finally {
      await server.close();
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('cleans up services that finish initializing after an immediate close', async (t) => {
    const dir = await mkdtemp(join(tmpdir(), 'mcp-init-close-'));
    const closedStores = [];
    const closeStore = A2AStore.prototype.close;
    t.mock.method(A2AStore.prototype, 'close', function () {
      closedStores.push(this);
      return closeStore.call(this);
    });
    let underlying;
    const server = createStatesetV2McpServer({
      createServer: (options) => {
        underlying = createStatesetMcpServer(options);
        return underlying;
      },
      commerce: {},
      dbPath: join(dir, 'store.db'),
      toolProfile: 'core',
    });
    try {
      await server.close();
      await underlying.servicesReady;
      assert.equal(closedStores.length, 1);
      assert.equal(closedStores[0].db, null);
    } finally {
      await server.close();
      await rm(dir, { recursive: true, force: true });
    }
  });
});
