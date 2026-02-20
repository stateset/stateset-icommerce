/**
 * Unit tests for mcp-server.js
 *
 * These tests verify the exports and basic structure of mcp-server.js.
 * Since the module uses the Claude Agent SDK which wraps tools internally,
 * we test what's actually exported: createStatesetMcpServer and TOOL_NAMES.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createStatesetMcpServer, TOOL_NAMES } from '../../src/mcp-server.js';

describe('mcp-server', () => {
  let mockCommerce;

  beforeEach(() => {
    // Create a minimal mock commerce instance
    mockCommerce = {
      customers: { list: async () => [], get: async () => null },
      orders: { list: async () => [], get: async () => null },
      products: { list: async () => [], get: async () => null },
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
      assert.equal(Array.isArray(payload), true);
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

      assert.equal(Array.isArray(payload), true);
      assert.equal(payload.length, 1);
      assert.equal(payload[0].sessionId, 'session-1');
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
    });
  });
});
