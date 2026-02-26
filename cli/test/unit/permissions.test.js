/**
 * Unit tests for permissions.js
 */

import { describe, it, after, beforeEach } from 'node:test';
import assert from 'node:assert';
import {
  PERMISSION_LEVELS,
  TOOL_PERMISSIONS,
  DEFAULT_GUARDRAILS,
  GuardrailsSchema,
  PermissionGate,
  createPermissionGate,
  getLevelFromFlags,
} from '../../src/permissions.js';
import { resetAuditStore } from '../../src/audit-store.js';

describe('permissions', () => {
  describe('PERMISSION_LEVELS', () => {
    it('should have all levels defined', () => {
      assert.strictEqual(PERMISSION_LEVELS.none, 0);
      assert.strictEqual(PERMISSION_LEVELS.read, 1);
      assert.strictEqual(PERMISSION_LEVELS.preview, 2);
      assert.strictEqual(PERMISSION_LEVELS.write, 3);
      assert.strictEqual(PERMISSION_LEVELS.delete, 4);
      assert.strictEqual(PERMISSION_LEVELS.admin, 5);
    });

    it('should have ascending order', () => {
      const levels = Object.values(PERMISSION_LEVELS);
      for (let i = 1; i < levels.length; i++) {
        assert.ok(levels[i] >= levels[i - 1], 'Levels should be in ascending order');
      }
    });
  });

  describe('TOOL_PERMISSIONS', () => {
    it('should have permissions for common tools', () => {
      assert.ok(TOOL_PERMISSIONS.list_customers);
      assert.ok(TOOL_PERMISSIONS.create_customer);
      assert.ok(TOOL_PERMISSIONS.list_orders);
      assert.ok(TOOL_PERMISSIONS.cancel_order);
    });

    it('should have read-only permissions for list/get operations', () => {
      assert.strictEqual(TOOL_PERMISSIONS.list_customers, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.get_customer, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.list_orders, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.get_order, 'read');
    });

    it('should have write permissions for create operations', () => {
      assert.strictEqual(TOOL_PERMISSIONS.create_customer, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.create_order, 'write');
    });

    it('should have delete permissions for cancel operations', () => {
      assert.strictEqual(TOOL_PERMISSIONS.cancel_order, 'delete');
    });

    it('should include provider-backed payment intent permissions', () => {
      assert.strictEqual(TOOL_PERMISSIONS.list_payment_providers, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.create_payment_intent, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.get_payment_intent, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.list_payment_intents, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.list_payment_settlements, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.list_payment_settlement_batches, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.reconcile_payment_provider, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.create_payment_settlement_batch, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.capture_payment_intent, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.cancel_payment_intent, 'delete');
      assert.strictEqual(TOOL_PERMISSIONS.refund_payment_intent, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.ingest_payment_provider_webhook, 'write');
    });

    it('should include provider-backed shipping and tax permissions', () => {
      assert.strictEqual(TOOL_PERMISSIONS.list_shipping_providers, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.quote_shipping_rates, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.create_shipping_label, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.void_shipping_label, 'delete');
      assert.strictEqual(TOOL_PERMISSIONS.track_shipping_label, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.list_shipping_labels, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.ingest_shipping_provider_webhook, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.handle_fulfillment_exception, 'write');

      assert.strictEqual(TOOL_PERMISSIONS.list_tax_providers, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.validate_tax_jurisdiction_compliance, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.calculate_tax_quote, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.calculate_tax_quote_with_failover, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.get_tax_quote, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.commit_tax_transaction, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.get_tax_transaction, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.list_tax_transactions, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.void_tax_transaction, 'delete');
      assert.strictEqual(TOOL_PERMISSIONS.ingest_tax_provider_webhook, 'write');
    });

    it('should include wasm connector ecosystem permissions', () => {
      assert.strictEqual(TOOL_PERMISSIONS.list_connector_marketplace, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.publish_wasm_connector, 'admin');
      assert.strictEqual(TOOL_PERMISSIONS.install_wasm_connector, 'write');
      assert.strictEqual(TOOL_PERMISSIONS.assess_wasm_connector_safety, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.certify_wasm_connector, 'admin');
      assert.strictEqual(TOOL_PERMISSIONS.sign_wasm_connector_attestation, 'admin');
      assert.strictEqual(TOOL_PERMISSIONS.verify_wasm_connector_attestation, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.uninstall_wasm_connector, 'delete');
      assert.strictEqual(TOOL_PERMISSIONS.list_installed_connectors, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.get_installed_connector, 'read');
      assert.strictEqual(TOOL_PERMISSIONS.execute_wasm_connector, 'write');
    });

    it('should treat x402 on-chain settlement as a write operation', () => {
      assert.strictEqual(TOOL_PERMISSIONS.x402_settle_intent_onchain, 'write');
    });

    it('should treat x402 incoming settlement recording as a write operation', () => {
      assert.strictEqual(TOOL_PERMISSIONS.x402_record_incoming_settlement, 'write');
    });

    it('should treat x402 end-to-end execution as a write operation', () => {
      assert.strictEqual(TOOL_PERMISSIONS.x402_execute_agent_payment, 'write');
    });
  });

  describe('PermissionGate', () => {
    let gate;

    beforeEach(() => {
      gate = new PermissionGate({ level: 'write' });
    });

    describe('checkPermission', () => {
      it('should allow read operations at write level', async () => {
        const result = await gate.checkPermission('list_customers', {});
        assert.strictEqual(result.allowed, true);
      });

      it('should allow write operations at write level', async () => {
        const result = await gate.checkPermission('create_customer', {});
        assert.strictEqual(result.allowed, true);
      });

      it('should deny admin operations at write level', async () => {
        const result = await gate.checkPermission('set_exchange_rate', {});
        assert.strictEqual(result.allowed, false);
      });

      it('should normalize tool names with mcp prefix', async () => {
        const result = await gate.checkPermission('mcp__stateset-commerce__list_customers', {});
        assert.strictEqual(result.allowed, true);
      });
    });

    describe('preview mode', () => {
      beforeEach(() => {
        gate = new PermissionGate({ level: 'preview' });
      });

      it('should allow read operations', async () => {
        const result = await gate.checkPermission('list_customers', {});
        assert.strictEqual(result.allowed, true);
      });

      it('should return preview info for write operations', async () => {
        const result = await gate.checkPermission('create_customer', { email: 'test@example.com' });
        assert.strictEqual(result.allowed, false);
        assert.strictEqual(result.preview, true);
        assert.ok(result.wouldDo);
      });
    });

    describe('rate limiting', () => {
      it('should track tool calls', async () => {
        for (let i = 0; i < 5; i++) {
          await gate.checkPermission('list_customers', {});
        }

        const summary = gate.getSummary();
        assert.ok(summary.rateLimits.toolCallsLastMinute >= 5);
      });
    });

    describe('audit logging', () => {
      it('should log allowed operations', async () => {
        await gate.checkPermission('list_customers', {});
        const log = gate.getAuditLog();
        assert.ok(log.length > 0);
        assert.strictEqual(log[0].tool, 'list_customers');
      });

      it('should sanitize sensitive params', async () => {
        await gate.checkPermission('create_customer', {
          email: 'test@example.com',
          password: 'secret',
        });
        const log = gate.getAuditLog();
        assert.strictEqual(log[0].params.password, '[REDACTED]');
      });
    });

    describe('guardrails', () => {
      it('should enforce inventory adjustment limits', async () => {
        gate = new PermissionGate({
          level: 'write',
          guardrails: { maxInventoryAdjustment: 100 },
        });

        const result = await gate.checkPermission('adjust_inventory', { quantity: 500 });
        assert.strictEqual(result.allowed, false);
        assert.ok(result.reason.includes('exceeds maximum'));
      });
    });
  });

  describe('createPermissionGate', () => {
    it('should create gate with preview level by default', () => {
      const gate = createPermissionGate({});
      assert.strictEqual(gate.getLevelName(), 'preview');
    });

    it('should create gate with write level when apply is true', () => {
      const gate = createPermissionGate({ apply: true });
      assert.strictEqual(gate.getLevelName(), 'write');
    });

    it('should create gate with admin level when admin is true', () => {
      const gate = createPermissionGate({ admin: true });
      assert.strictEqual(gate.getLevelName(), 'admin');
    });

    it('should create gate with read level when readonly is true', () => {
      const gate = createPermissionGate({ readonly: true });
      assert.strictEqual(gate.getLevelName(), 'read');
    });
  });

  describe('getLevelFromFlags', () => {
    it('should return preview by default', () => {
      assert.strictEqual(getLevelFromFlags({}), 'preview');
    });

    it('should return write when apply is set', () => {
      assert.strictEqual(getLevelFromFlags({ apply: true }), 'write');
    });

    it('should return admin when admin is set', () => {
      assert.strictEqual(getLevelFromFlags({ admin: true }), 'admin');
    });

    it('should prioritize admin over apply', () => {
      assert.strictEqual(getLevelFromFlags({ apply: true, admin: true }), 'admin');
    });
  });

  // --------------------------------------------------------------------------
  // GuardrailsSchema validation
  // --------------------------------------------------------------------------

  describe('GuardrailsSchema', () => {
    after(() => resetAuditStore());

    it('should accept valid config', () => {
      const result = GuardrailsSchema.safeParse({
        maxOrderValue: 5000,
        maxToolCallsPerMinute: 60,
      });
      assert.strictEqual(result.success, true);
    });

    it('should reject negative maxOrderValue', () => {
      const result = GuardrailsSchema.safeParse({ maxOrderValue: -100 });
      assert.strictEqual(result.success, false);
    });

    it('should reject non-integer maxToolCallsPerMinute', () => {
      const result = GuardrailsSchema.safeParse({ maxToolCallsPerMinute: 1.5 });
      assert.strictEqual(result.success, false);
    });

    it('should reject zero maxToolCallsPerMinute', () => {
      const result = GuardrailsSchema.safeParse({ maxToolCallsPerMinute: 0 });
      assert.strictEqual(result.success, false);
    });

    it('should apply defaults for missing fields', () => {
      const result = GuardrailsSchema.parse({});
      assert.strictEqual(result.maxOrderValue, 10000);
      assert.strictEqual(result.maxToolCallsPerMinute, 120);
      assert.strictEqual(result.confirmBulkOperations, true);
      assert.deepStrictEqual(result.blockedTools, []);
    });

    it('should fall back to defaults on invalid config in constructor', () => {
      const gate = new PermissionGate({
        level: 'read',
        guardrails: { maxToolCallsPerMinute: -5 },
      });
      assert.strictEqual(
        gate.guardrails.maxToolCallsPerMinute,
        DEFAULT_GUARDRAILS.maxToolCallsPerMinute,
      );
    });

    it('default guardrails require approval for x402 on-chain settlement', () => {
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('x402_settle_intent_onchain'));
    });

    it('default guardrails require approval for x402 incoming settlement recording', () => {
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('x402_record_incoming_settlement'));
    });

    it('default guardrails require approval for x402 end-to-end execution', () => {
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('x402_execute_agent_payment'));
    });

    it('default guardrails require approval for payment intent capture and cancellation', () => {
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('create_payment_settlement_batch'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('capture_payment_intent'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('cancel_payment_intent'));
    });

    it('default guardrails require approval for fulfillment exception workflow execution', () => {
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('handle_fulfillment_exception'));
    });

    it('default guardrails require approval for connector publish/install/certify/sign/uninstall/execute', () => {
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('publish_wasm_connector'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('install_wasm_connector'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('certify_wasm_connector'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('sign_wasm_connector_attestation'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('uninstall_wasm_connector'));
      assert.ok(DEFAULT_GUARDRAILS.requireApprovalFor.includes('execute_wasm_connector'));
    });
  });

  // --------------------------------------------------------------------------
  // Blocked tools
  // --------------------------------------------------------------------------

  describe('blocked tools', () => {
    after(() => resetAuditStore());

    it('should deny blocked tools even at admin level', async () => {
      const gate = new PermissionGate({
        level: 'admin',
        guardrails: { blockedTools: ['create_order'] },
      });
      const result = await gate.checkPermission('create_order');
      assert.strictEqual(result.allowed, false);
      assert.ok(result.reason.includes('blocked by policy'));
    });
  });

  // --------------------------------------------------------------------------
  // Rate limit enforcement
  // --------------------------------------------------------------------------

  describe('rate limit enforcement', () => {
    after(() => resetAuditStore());

    it('should enforce tool calls per minute limit', async () => {
      const gate = new PermissionGate({
        level: 'read',
        guardrails: { maxToolCallsPerMinute: 3 },
      });

      for (let i = 0; i < 3; i++) {
        const r = await gate.checkPermission('list_customers');
        assert.strictEqual(r.allowed, true, `call ${i + 1} should be allowed`);
      }

      const denied = await gate.checkPermission('list_customers');
      assert.strictEqual(denied.allowed, false);
      assert.ok(denied.reason.includes('Rate limit'));
    });

    it('should enforce write ops per minute limit', async () => {
      const gate = new PermissionGate({
        level: 'write',
        guardrails: { maxWriteOpsPerMinute: 2, maxToolCallsPerMinute: 100 },
      });

      for (let i = 0; i < 2; i++) {
        const r = await gate.checkPermission('create_customer');
        assert.strictEqual(r.allowed, true);
      }

      const denied = await gate.checkPermission('create_customer');
      assert.strictEqual(denied.allowed, false);
      assert.ok(denied.reason.includes('write operations'));
    });
  });

  // --------------------------------------------------------------------------
  // Approval required
  // --------------------------------------------------------------------------

  describe('requireApprovalFor', () => {
    after(() => resetAuditStore());

    it('should deny when user declines confirmation', async () => {
      const gate = new PermissionGate({
        level: 'delete',
        guardrails: { requireApprovalFor: ['cancel_order'] },
        onConfirmRequired: async () => false,
      });
      const result = await gate.checkPermission('cancel_order');
      assert.strictEqual(result.allowed, false);
      assert.ok(result.reason.includes('declined'));
    });

    it('should allow when user confirms', async () => {
      const gate = new PermissionGate({
        level: 'delete',
        guardrails: { requireApprovalFor: ['cancel_order'] },
        onConfirmRequired: async () => true,
      });
      const result = await gate.checkPermission('cancel_order');
      assert.strictEqual(result.allowed, true);
    });
  });

  // --------------------------------------------------------------------------
  // TOOL_PERMISSIONS completeness
  // --------------------------------------------------------------------------

  describe('TOOL_PERMISSIONS completeness', () => {
    it('should map all permission values to known levels', () => {
      const validLevels = new Set(Object.keys(PERMISSION_LEVELS));
      for (const [tool, level] of Object.entries(TOOL_PERMISSIONS)) {
        assert.ok(validLevels.has(level), `Tool '${tool}' has unknown level '${level}'`);
      }
    });
  });
});
