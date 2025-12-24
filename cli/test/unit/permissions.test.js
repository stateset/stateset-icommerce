/**
 * Unit tests for permissions.js
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import {
  PERMISSION_LEVELS,
  TOOL_PERMISSIONS,
  PermissionGate,
  createPermissionGate,
  getLevelFromFlags
} from '../../src/permissions.js';

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
        await gate.checkPermission('create_customer', { email: 'test@example.com', password: 'secret' });
        const log = gate.getAuditLog();
        assert.strictEqual(log[0].params.password, '[REDACTED]');
      });
    });

    describe('guardrails', () => {
      it('should enforce inventory adjustment limits', async () => {
        gate = new PermissionGate({
          level: 'write',
          guardrails: { maxInventoryAdjustment: 100 }
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
});
