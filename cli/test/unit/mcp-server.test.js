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
  });
});
