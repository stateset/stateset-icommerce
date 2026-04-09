/**
 * Unit tests for mcp-server.js
 *
 * These tests verify the exports and basic structure of mcp-server.js.
 * Since the module uses the Claude Agent SDK which wraps tools internally,
 * we test what's actually exported: createStatesetMcpServer and TOOL_NAMES.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createStatesetMcpServer, TOOL_NAMES } from '../../src/mcp-server.js';
import { createPaymentCredential } from '../../src/mpp/index.js';
import { loadTreasuryContext, recordDeposit } from '../../src/treasury/index.js';

async function withTempTreasuryFixture(callback) {
  const tempDir = await mkdtemp(join(os.tmpdir(), 'stateset-mpp-'));
  const pricingPath = join(tempDir, 'pricing.json');
  const dbPath = join(tempDir, 'treasury.db');

  await writeFile(
    pricingPath,
    JSON.stringify(
      {
        rules: [
          {
            tool: 'list_customers',
            chainId: 'bitcoin',
            tokenSymbol: 'BTC',
            amount: 0.0001,
          },
        ],
      },
      null,
      2,
    ),
  );

  try {
    return await callback({ tempDir, pricingPath, dbPath });
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function withTempDbFixture(callback) {
  const tempDir = await mkdtemp(join(os.tmpdir(), 'stateset-mcp-server-'));
  const dbPath = join(tempDir, 'store.db');

  try {
    return await callback({ tempDir, dbPath });
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function seedTreasuryBalance({ dbPath, pricingPath, agentId = 'buyer-agent', amount = 1 }) {
  const ctx = await loadTreasuryContext({ dbPath, pricingPath });
  await recordDeposit(
    {
      agentId,
      chainId: 'bitcoin',
      tokenSymbol: 'BTC',
      amount,
      source: 'test_seed',
    },
    ctx,
  );
}

describe('mcp-server', () => {
  let mockCommerce;

  beforeEach(() => {
    // Create a minimal mock commerce instance
    mockCommerce = {
      customers: {
        list: async () => [],
        count: async () => 0,
        get: async () => null,
        create: async (data) => ({ id: 'cust-1', ...data }),
      },
      orders: { list: async () => [], count: async () => 0, get: async () => null },
      products: { list: async () => [], count: async () => 0, get: async () => null },
      inventory: { getStock: async () => null },
    };
  });

  describe('Module exports', () => {
    it('should export createStatesetMcpServer function', () => {
      assert.strictEqual(typeof createStatesetMcpServer, 'function');
    });

    it('should export TOOL_NAMES array', () => {
      assert.ok(Array.isArray(TOOL_NAMES));
    });

    it('should have many tools in TOOL_NAMES', () => {
      // Should have tools from all 26 modules (customers, orders, products, inventory, etc.)
      assert.ok(TOOL_NAMES.length >= 100, `Expected at least 100 tools, got ${TOOL_NAMES.length}`);
    });
  });

  describe('TOOL_NAMES format', () => {
    it('should format all tool names as mcp__stateset-commerce__<name>', () => {
      TOOL_NAMES.forEach((name) => {
        assert.ok(
          name.startsWith('mcp__stateset-commerce__'),
          `Tool name ${name} should start with mcp__stateset-commerce__`,
        );
      });
    });

    it('should include common customer tools', () => {
      const toolNameSet = new Set(TOOL_NAMES);
      assert.ok(toolNameSet.has('mcp__stateset-commerce__list_customers'));
      assert.ok(toolNameSet.has('mcp__stateset-commerce__get_customer'));
      assert.ok(toolNameSet.has('mcp__stateset-commerce__create_customer'));
    });

    it('should include common order tools', () => {
      const toolNameSet = new Set(TOOL_NAMES);
      assert.ok(toolNameSet.has('mcp__stateset-commerce__list_orders'));
      assert.ok(toolNameSet.has('mcp__stateset-commerce__get_order'));
      assert.ok(toolNameSet.has('mcp__stateset-commerce__create_order'));
    });

    it('should include tools from all domain modules', () => {
      const toolNameStr = TOOL_NAMES.join(' ');
      // Check for at least one tool from each major domain
      const domains = [
        'customer',
        'order',
        'product',
        'inventory',
        'return',
        'cart',
        'sales', // analytics tools use 'sales', 'forecast', 'metrics'
        'currency',
        'tax',
        'promotion',
        'subscription',
        'bom', // manufacturing tools use 'bom' and 'work_order'
        'payment',
        'shipment',
        'supplier',
        'invoice',
        'warranty',
        'vector',
      ];

      domains.forEach((domain) => {
        assert.ok(
          toolNameStr.includes(domain),
          `Should have at least one tool from ${domain} module`,
        );
      });
    });
  });

  describe('createStatesetMcpServer', () => {
    it('should create server with required commerce parameter', () => {
      const server = createStatesetMcpServer({ commerce: mockCommerce });

      assert.ok(server);
      assert.strictEqual(server.type, 'sdk');
      assert.strictEqual(server.name, 'stateset-commerce');
      assert.ok(server.instance);
    });

    it('should create a working server from dbPath without an explicit commerce instance', async () => {
      await withTempDbFixture(async ({ dbPath }) => {
        const server = createStatesetMcpServer({ dbPath, allowApply: false });
        const result = await server.executeTool('list_customers');

        assert.equal(result.success, true);
        assert.equal(result.status, 'success');
        assert.equal(result.result.count, 0);
        assert.deepEqual(result.result.customers, []);
      });
    });

    it('should accept allowApply parameter', () => {
      const server1 = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: false,
      });

      const server2 = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
      });

      assert.ok(server1);
      assert.ok(server2);
    });

    it('should accept optional telemetry parameter', () => {
      const mockTelemetry = {
        logToolCall: () => {},
        logCustomEvent: () => {},
      };

      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        telemetry: mockTelemetry,
      });

      assert.ok(server);
    });

    it('should accept optional permissionGate parameter', () => {
      const mockPermissionGate = {
        checkPermission: async () => ({ allowed: true }),
      };

      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        permissionGate: mockPermissionGate,
      });

      assert.ok(server);
    });

    it('should accept optional hookRunner parameter', () => {
      const mockHookRunner = {
        hasHooks: () => false,
        run: async () => ({}),
      };

      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        hookRunner: mockHookRunner,
      });

      assert.ok(server);
    });

    it('should accept optional dbPath parameter', () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        dbPath: './test-store.db',
      });

      assert.ok(server);
    });

    it('should accept optional treasury parameter', () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        treasury: {
          agentId: 'test-agent',
          dbPath: './test-treasury.db',
        },
      });

      assert.ok(server);
    });

    it('should accept all optional parameters', () => {
      const mockTelemetry = {
        logToolCall: () => {},
        logCustomEvent: () => {},
      };

      const mockPermissionGate = {
        checkPermission: async () => ({ allowed: true }),
      };

      const mockHookRunner = {
        hasHooks: () => false,
        run: async () => ({}),
      };

      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
        telemetry: mockTelemetry,
        permissionGate: mockPermissionGate,
        hookRunner: mockHookRunner,
        dbPath: './test-store.db',
        treasury: {
          agentId: 'test-agent',
          dbPath: './test-treasury.db',
          erc8004Registry: 'test-registry',
          erc8004DbPath: './test-identity.db',
        },
      });

      assert.ok(server);
      assert.strictEqual(server.name, 'stateset-commerce');
      assert.ok(server.instance);
    });

    it('should preserve declared policy domains for agentic runtime tools', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
        autonomousEngine: {
          executeAgentRequest: async () => ({ status: 'completed' }),
        },
      });

      const result = await server.executeTool('delegate_to_agent', {
        agent_name: 'orders',
        task_description: 'Review pending orders over $500',
        context: { limit: 10 },
      });

      assert.equal(result.status, 'success');
      assert.equal(result.policy.domain, 'agentic');
      assert.equal(result.runtime.policyDomain, 'agentic');
    });

    it('should expose the active MCP event stream on server object', () => {
      const server = createStatesetMcpServer({ commerce: mockCommerce });
      assert.ok(server.mcpEventStream);
      assert.strictEqual(typeof server.mcpEventStream.publish, 'function');
    });

    it('should accept custom MCP event stream injection', () => {
      const customStream = {
        publish: () => {},
        subscribe: async () => ({ success: true }),
        unsubscribe: async () => ({ success: true }),
        listSubscriptions: async () => [],
        getEventHistory: async () => [],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: customStream,
      });
      assert.strictEqual(server.mcpEventStream, customStream);
    });

    it('should include _agentic metadata when structuredToolResults is enabled', async () => {
      const subscriptions = [];
      const customStream = {
        publish: () => {},
        subscribe: async ({ sessionId, eventTypes }) => {
          const result = {
            success: true,
            subscription: {
              id: 'sub-structured',
              sessionId: sessionId || '__global__',
              eventTypes,
              active: true,
              createdAt: new Date().toISOString(),
              lastEventId: null,
            },
          };
          subscriptions.push(result.subscription);
          return result;
        },
        unsubscribe: async () => ({ success: true }),
        listSubscriptions: async () => subscriptions,
        getEventHistory: async () => [],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        structuredToolResults: true,
        mcpEventStream: customStream,
      });
      const tool = server.instance._registeredTools.agentic_subscribe_events;
      const res = await tool.handler({ sessionId: 'session-structured', eventTypes: ['success'] });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.success, true);
      assert.equal(payload._agentic?.tool, 'agentic_subscribe_events');
      assert.equal(payload._agentic?.sessionId, 'session-structured');
      assert.equal(payload._agentic?.status, 'success');
      assert.equal(typeof payload._agentic?.timing?.elapsedMs, 'number');
      assert.equal(payload._agentic?.schemaVersion, '1.0.0');
    });

    it('should not include _agentic metadata by default', async () => {
      const customStream = {
        publish: () => {},
        subscribe: async () => ({ success: true }),
        unsubscribe: async () => ({ success: true }),
        listSubscriptions: async () => [],
        getEventHistory: async () => [],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: customStream,
      });
      const tool = server.instance._registeredTools.agentic_list_event_subscriptions;
      const res = await tool.handler({ sessionId: 'session-legacy' });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload._agentic, undefined);
      assert.equal(Array.isArray(payload.subscriptions), true);
      assert.equal(payload.count, 0);
    });

    it('should expose agentic tool result schema contract from runtime tool', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
      });
      const tool = server.instance._registeredTools.agentic_runtime_contract;
      const res = await tool.handler({});
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.engine, 'stateset-icommerce');
      assert.equal(payload.agenticToolResultSchema.version, '1.0.0');
      assert.equal(payload.agenticToolResultSchema.envelope, 'mcp_tool_result');
      assert.equal(Array.isArray(payload.agenticToolResultSchema.metadata), true);
      assert.equal(payload.agenticToolResultSchema.metadata.includes('schemaVersion'), true);
      assert.equal(payload.agenticToolResultSchema.metadata.includes('mutation'), true);
      assert.equal(payload.mpp.transport.jsonrpc.paymentRequiredCode, -32042);
      assert.equal(Array.isArray(payload.mpp.methodAdapters), true);
    });

    it('should expose OpenAPI payment discovery for priced tools', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const discovery = await server.getPaymentDiscovery({
          format: 'openapi',
          tool: 'list_customers',
        });

        assert.equal(discovery.openapi, '3.1.0');
        assert.equal(discovery['x-service-info'].protocol, 'mpp');
        assert.equal(
          discovery.paths['/mcp/tools/list_customers'].post['x-payment-info'].amount.asset,
          'BTC',
        );
      });
    });

    it('should expose agentic_tool_catalog with payable tool metadata', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const tool = server.instance._registeredTools.agentic_tool_catalog;
        const res = await tool.handler({ tool: 'list_customers', payableOnly: true });
        const payload = JSON.parse(res.content[0].text);

        assert.equal(payload.format, 'generic');
        assert.equal(payload.count, 1);
        assert.equal(payload.tools[0].toolName, 'list_customers');
        assert.equal(payload.tools[0].payable, true);
        assert.equal(payload.tools[0].paymentInfo.amount.asset, 'BTC');
      });
    });

    it('should return an MPP payment challenge for priced tools without credentials', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const tool = server.instance._registeredTools.list_customers;
        const res = await tool.handler({}, { requestId: 'req-mpp-1', sessionId: 'sess-mpp-1' });
        const payload = JSON.parse(res.content[0].text);

        assert.equal(res.isError, true);
        assert.equal(payload.code, -32042);
        assert.equal(payload.paymentRequired, true);
        assert.equal(payload.paymentChallenge.tool, 'list_customers');
        assert.equal(payload._meta.payment.challenge.challengeId, payload.paymentChallenge.challengeId);
      });
    });

    it('should accept an MPP credential and attach a receipt for priced tools', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        await seedTreasuryBalance({ dbPath, pricingPath });

        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const tool = server.instance._registeredTools.list_customers;
        const challengeRes = await tool.handler(
          {},
          { requestId: 'req-mpp-2', sessionId: 'sess-mpp-2' },
        );
        const challengePayload = JSON.parse(challengeRes.content[0].text);
        const credential = createPaymentCredential({
          challenge: challengePayload.paymentChallenge,
          payer: 'buyer-agent',
          authorization: { type: 'test' },
        });

        const successRes = await tool.handler(
          {},
          {
            requestId: 'req-mpp-2',
            sessionId: 'sess-mpp-2',
            _meta: { payment: credential },
          },
        );
        const successPayload = JSON.parse(successRes.content[0].text);

        assert.equal(successRes.isError, undefined);
        assert.equal(Array.isArray(successPayload.customers), true);
        assert.equal(
          successPayload._meta.payment.receipt.challengeId,
          challengePayload.paymentChallenge.challengeId,
        );
        assert.equal(successPayload._meta.payment.credentialId, credential.credentialId);
      });
    });

    it('should execute priced tools with automatic MPP payment retry', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        await seedTreasuryBalance({ dbPath, pricingPath });

        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const result = await server.executeToolWithPayment('list_customers', {}, {
          payment: {
            payer: 'buyer-agent',
            acceptedMethods: ['bitcoin'],
            maxAmountSmallest: '10000',
          },
        });

        assert.equal(result.success, true);
        assert.equal(result.status, 'success');
        assert.equal(Array.isArray(result.result.customers), true);
        assert.equal(result.result._meta.payment.receipt.tool, 'list_customers');
        assert.equal(result.result._meta.payment.receipt.payer, 'buyer-agent');
      });
    });

    it('should expose agentic_payment_discovery for payable-tool lookup', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const tool = server.instance._registeredTools.agentic_payment_discovery;
        const res = await tool.handler({ pricedOnly: true });
        const payload = JSON.parse(res.content[0].text);

        assert.equal(payload.protocol, 'mpp');
        assert.equal(Array.isArray(payload.tools), true);
        assert.equal(payload.tools[0].name, 'list_customers');
        assert.equal(payload.tools[0].paymentInfo.amount.asset, 'BTC');
      });
    });

    it('should return payment-aware tool discovery search results', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const tool = server.instance._registeredTools.discover_tools;
        const res = await tool.handler({ intent: 'customer', limit: 5 });
        const payload = JSON.parse(res.content[0].text);
        const payable = payload.tools.find((entry) => entry.name === 'list_customers');

        assert.equal(payload.success, true);
        assert.equal(payable.payable, true);
        assert.equal(payable.paymentInfo.amount.asset, 'BTC');
      });
    });

    it('should expose agentic_prepare_payment with challenge and retry template', async () => {
      await withTempTreasuryFixture(async ({ pricingPath, dbPath }) => {
        const server = createStatesetMcpServer({
          commerce: mockCommerce,
          treasury: {
            enabled: true,
            agentId: 'buyer-agent',
            dbPath,
            pricingPath,
          },
        });

        const tool = server.instance._registeredTools.agentic_prepare_payment;
        const res = await tool.handler({
          tool: 'list_customers',
          params: {},
          requestId: 'req-prep-1',
          sessionId: 'sess-prep-1',
        });
        const payload = JSON.parse(res.content[0].text);

        assert.equal(payload.success, true);
        assert.equal(payload.payable, true);
        assert.equal(payload.challenge.tool, 'list_customers');
        assert.equal(payload.retryExample._meta.payment.challengeId, payload.challenge.challengeId);
      });
    });
  });

  describe('Agentic event stream tools', () => {
    it('agentic_subscribe_events returns stream subscription output', async () => {
      const subscriptions = [];
      const customStream = {
        publish: () => {},
        subscribe: async ({ sessionId, eventTypes }) => {
          const result = {
            success: true,
            subscription: {
              id: 'sub-1',
              sessionId: sessionId || '__global__',
              eventTypes,
              active: true,
              createdAt: new Date().toISOString(),
              lastEventId: null,
            },
          };
          subscriptions.push(result.subscription);
          return result;
        },
        unsubscribe: async () => ({ success: true }),
        listSubscriptions: async () => subscriptions,
        getEventHistory: async () => [],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: customStream,
      });
      const tool = server.instance._registeredTools.agentic_subscribe_events;
      const res = await tool.handler({ sessionId: 'session-123', eventTypes: ['success', 'error'] });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.success, true);
      assert.equal(payload.subscription.sessionId, 'session-123');
      assert.deepEqual(payload.subscription.eventTypes, ['success', 'error']);
      assert.equal(payload.subscription.active, true);
    });

    it('agentic_unsubscribe_events handles missing subscriptions from stream service', async () => {
      const customStream = {
        publish: () => {},
        subscribe: async () => ({ success: true }),
        unsubscribe: async () => ({ success: false, error: 'Subscription not found' }),
        listSubscriptions: async () => [],
        getEventHistory: async () => [],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: customStream,
      });
      const tool = server.instance._registeredTools.agentic_unsubscribe_events;
      const res = await tool.handler({ subscriptionId: 'missing-id' });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.success, false);
      assert.equal(payload.error, 'Subscription not found');
    });

    it('agentic_list_event_subscriptions forwards to stream service', async () => {
      const customStream = {
        publish: () => {},
        subscribe: async () => ({ success: true }),
        unsubscribe: async () => ({ success: true }),
        listSubscriptions: async ({ sessionId } = {}) => [
          { id: '1', sessionId: sessionId || '__global__', eventTypes: ['success'], active: true },
        ],
        getEventHistory: async () => [],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: customStream,
      });
      const tool = server.instance._registeredTools.agentic_list_event_subscriptions;
      const res = await tool.handler({ sessionId: 'session-1' });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(Array.isArray(payload.subscriptions), true);
      assert.equal(payload.count, 1);
      assert.equal(payload.subscriptions[0].sessionId, 'session-1');
    });

    it('agentic_get_event_history forwards to stream service', async () => {
      const customStream = {
        publish: () => {},
        subscribe: async () => ({ success: true }),
        unsubscribe: async () => ({ success: true }),
        listSubscriptions: async () => [],
        getEventHistory: async ({ sessionId, limit }) => [
          { id: 'e1', sessionId: sessionId || '__global__', type: 'success', limit },
        ],
      };
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: customStream,
      });
      const tool = server.instance._registeredTools.agentic_get_event_history;
      const res = await tool.handler({ sessionId: 'session-2', limit: 5 });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(Array.isArray(payload), true);
      assert.equal(payload.length, 1);
      assert.equal(payload[0].sessionId, 'session-2');
      assert.equal(payload[0].type, 'success');
      assert.equal(payload[0].limit, 5);
    });

    it('returns unavailable message if mcp event stream is missing', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        mcpEventStream: {},
      });
      const tool = server.instance._registeredTools.agentic_list_event_subscriptions;
      const res = await tool.handler({});
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.success, false);
      assert.equal(payload.error, 'MCP event stream service is unavailable');
    });
  });

  describe('Agentic mutation simulation and replay tools', () => {
    it('agentic_simulate_mutation returns deterministic dry-run metadata', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
      });
      const tool = server.instance._registeredTools.agentic_simulate_mutation;
      const res = await tool.handler({
        tool: 'create_customer',
        params: {
          email: 'simulate@example.com',
          firstName: 'Sim',
          lastName: 'User',
        },
      });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.success, true);
      assert.equal(payload.targetTool, 'create_customer');
      assert.equal(payload.outcome.status, 'dry_run_success');
      assert.ok(payload.outcome.mutationManifest);
      assert.ok(payload.outcome.policy?.decisionBundle?.auditArtifact?.signature);
    });

    it('agentic_replay_mutation replays latest write event in dry-run mode', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
      });

      const createCustomer = server.instance._registeredTools.create_customer;
      await createCustomer.handler({
        email: 'replay@example.com',
        firstName: 'Replay',
        lastName: 'User',
      });

      const replayTool = server.instance._registeredTools.agentic_replay_mutation;
      const replayRes = await replayTool.handler({
        tool: 'create_customer',
        dryRun: true,
      });
      const payload = JSON.parse(replayRes.content[0].text);

      assert.equal(payload.success, true);
      assert.equal(payload.sourceEvent.tool, 'create_customer');
      assert.ok(payload.replay);
      assert.equal(typeof payload.deterministic.paramsMatch, 'boolean');
    });
  });

  describe('Agentic plan SLA integration', () => {
    it('agentic_plan includes SLA-aware routing metadata', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
      });
      const tool = server.instance._registeredTools.agentic_plan;
      const res = await tool.handler({
        steps: [{ tool: 'list_orders', params: {} }],
        slaLevel: 'critical',
      });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.slaLevel, 'critical');
      assert.equal(Array.isArray(payload.outcomes), true);
      assert.equal(payload.outcomes[0].routing.slaLevel, 'critical');
      assert.equal(payload.outcomes[0].routing.primary.agent, 'orders');
    });

    it('agentic_execute_plan resolves SLA template references and exposes routing metadata', async () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
      });
      const tool = server.instance._registeredTools.agentic_execute_plan;
      const res = await tool.handler({
        steps: [
          {
            tool: 'create_customer',
            params: {
              email: 'sla-template@example.com',
              firstName: '{{ slaLevel }}',
              lastName: 'User',
            },
          },
        ],
        dryRun: true,
        slaLevel: 'expedited',
      });
      const payload = JSON.parse(res.content[0].text);

      assert.equal(payload.slaLevel, 'expedited');
      assert.equal(payload.finalStatus, 'dry_run');
      assert.equal(payload.steps[0].status, 'dry_run_success');
      assert.equal(payload.steps[0].params.firstName, 'expedited');
      assert.equal(payload.steps[0].routing.slaLevel, 'expedited');
      assert.equal(typeof payload.steps[0].routing.primary.agent, 'string');
    });
  });

  describe('Server structure', () => {
    it('should have type property set to sdk', () => {
      const server = createStatesetMcpServer({ commerce: mockCommerce });
      assert.strictEqual(server.type, 'sdk');
    });

    it('should have name property set to stateset-commerce', () => {
      const server = createStatesetMcpServer({ commerce: mockCommerce });
      assert.strictEqual(server.name, 'stateset-commerce');
    });

    it('should have instance property with SDK internals', () => {
      const server = createStatesetMcpServer({ commerce: mockCommerce });

      assert.ok(server.instance);
      assert.strictEqual(typeof server.instance, 'object');
      // SDK internal properties
      assert.ok('_registeredTools' in server.instance);
      assert.ok('_toolHandlersInitialized' in server.instance);
    });

    it('should expose connect and close helpers on the returned server wrapper', async () => {
      const server = createStatesetMcpServer({ commerce: mockCommerce });
      const calls = [];
      server.instance.connect = async (...args) => {
        calls.push({ method: 'connect', args });
        return { connected: true };
      };
      server.instance.server.close = async (...args) => {
        calls.push({ method: 'close', args });
        return { closed: true };
      };

      const connectResult = await server.connect('transport-demo');
      const closeResult = await server.close('reason-demo');

      assert.deepEqual(connectResult, { connected: true });
      assert.deepEqual(closeResult, { closed: true });
      assert.deepEqual(calls, [
        { method: 'connect', args: ['transport-demo'] },
        { method: 'close', args: ['reason-demo'] },
      ]);
    });
  });

  describe('Integration', () => {
    it('should work with all parameters null except commerce', () => {
      const server = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: false,
        telemetry: null,
        permissionGate: null,
        hookRunner: null,
        dbPath: './store.db',
        treasury: null,
      });

      assert.ok(server);
      assert.ok(server.instance);
    });

    it('should create consistent server across multiple calls', () => {
      const server1 = createStatesetMcpServer({ commerce: mockCommerce });
      const server2 = createStatesetMcpServer({ commerce: mockCommerce });

      assert.strictEqual(server1.type, server2.type);
      assert.strictEqual(server1.name, server2.name);
      assert.ok(server1.instance);
      assert.ok(server2.instance);
    });

    it('should create different server instances for different configurations', () => {
      const server1 = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: false,
      });

      const server2 = createStatesetMcpServer({
        commerce: mockCommerce,
        allowApply: true,
      });

      assert.ok(server1);
      assert.ok(server2);
      // Different instances but same structure
      assert.strictEqual(server1.type, server2.type);
      assert.strictEqual(server1.name, server2.name);
    });
  });

  describe('Tool count verification', () => {
    it('should have tools from customers module', () => {
      const customerTools = TOOL_NAMES.filter((name) => name.includes('customer'));
      assert.ok(customerTools.length >= 2, `Expected at least 2 customer tools, got ${customerTools.length}`);
    });

    it('should have tools from orders module', () => {
      const orderTools = TOOL_NAMES.filter((name) => name.includes('order'));
      assert.ok(orderTools.length >= 2, `Expected at least 2 order tools, got ${orderTools.length}`);
    });

    it('should have tools from products module', () => {
      const productTools = TOOL_NAMES.filter((name) => name.includes('product'));
      assert.ok(productTools.length >= 2, `Expected at least 2 product tools, got ${productTools.length}`);
    });

    it('should have tools from inventory module', () => {
      const inventoryTools = TOOL_NAMES.filter((name) => name.includes('inventory'));
      assert.ok(inventoryTools.length >= 1, `Expected at least 1 inventory tool, got ${inventoryTools.length}`);
    });

    it('should have tools from carts module', () => {
      const cartTools = TOOL_NAMES.filter((name) => name.includes('cart'));
      assert.ok(cartTools.length >= 2, `Expected at least 2 cart tools, got ${cartTools.length}`);
    });

    it('should have tools from returns module', () => {
      const returnTools = TOOL_NAMES.filter((name) => name.includes('return'));
      assert.ok(returnTools.length >= 1, `Expected at least 1 return tool, got ${returnTools.length}`);
    });

    it('should have tools from analytics module', () => {
      const analyticsTools = TOOL_NAMES.filter(
        (name) => name.includes('analytics') || name.includes('sales') || name.includes('forecast'),
      );
      assert.ok(analyticsTools.length >= 1, `Expected at least 1 analytics tool, got ${analyticsTools.length}`);
    });

    it('should have tools from payments module', () => {
      const paymentTools = TOOL_NAMES.filter((name) => name.includes('payment'));
      assert.ok(paymentTools.length >= 1, `Expected at least 1 payment tool, got ${paymentTools.length}`);
    });

    it('should have tools from promotions module', () => {
      const promotionTools = TOOL_NAMES.filter(
        (name) => name.includes('promotion') || name.includes('coupon'),
      );
      assert.ok(promotionTools.length >= 1, `Expected at least 1 promotion tool, got ${promotionTools.length}`);
    });

    it('should have tools from subscriptions module', () => {
      const subscriptionTools = TOOL_NAMES.filter((name) => name.includes('subscription'));
      assert.ok(subscriptionTools.length >= 1, `Expected at least 1 subscription tool, got ${subscriptionTools.length}`);
    });

    it('should include event stream agentic tools', () => {
      const eventTools = TOOL_NAMES.filter((name) => name.includes('agentic_subscribe_events'));
      assert.ok(eventTools.length >= 1, `Expected event subscription tool, got ${eventTools.length}`);
      assert.ok(TOOL_NAMES.includes('mcp__stateset-commerce__agentic_unsubscribe_events'));
      assert.ok(TOOL_NAMES.includes('mcp__stateset-commerce__agentic_list_event_subscriptions'));
      assert.ok(TOOL_NAMES.includes('mcp__stateset-commerce__agentic_get_event_history'));
      assert.ok(TOOL_NAMES.includes('mcp__stateset-commerce__agentic_simulate_mutation'));
      assert.ok(TOOL_NAMES.includes('mcp__stateset-commerce__agentic_replay_mutation'));
    });
  });
});
