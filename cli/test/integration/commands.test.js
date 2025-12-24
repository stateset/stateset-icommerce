/**
 * Integration tests for command modules
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import {
  commands,
  expandResource,
  expandAction,
  getCommand,
  RESOURCE_ALIASES,
  ACTION_ALIASES
} from '../../src/commands/index.js';

describe('commands integration', () => {
  describe('command registry', () => {
    it('should have all resource modules', () => {
      assert.ok(commands.customers, 'customers module should exist');
      assert.ok(commands.orders, 'orders module should exist');
      assert.ok(commands.products, 'products module should exist');
      assert.ok(commands.inventory, 'inventory module should exist');
      assert.ok(commands.returns, 'returns module should exist');
    });

    it('should have execute function for each command', () => {
      for (const [name, cmd] of Object.entries(commands)) {
        assert.strictEqual(typeof cmd.execute, 'function', `${name} should have execute function`);
      }
    });

    it('should have metadata for each command', () => {
      for (const [name, cmd] of Object.entries(commands)) {
        assert.ok(cmd.metadata, `${name} should have metadata`);
        assert.ok(cmd.metadata.name, `${name} should have metadata.name`);
        assert.ok(cmd.metadata.aliases, `${name} should have metadata.aliases`);
        assert.ok(cmd.metadata.actions, `${name} should have metadata.actions`);
      }
    });
  });

  describe('resource aliases', () => {
    it('should have single letter aliases', () => {
      assert.strictEqual(RESOURCE_ALIASES['c'], 'customers');
      assert.strictEqual(RESOURCE_ALIASES['o'], 'orders');
      assert.strictEqual(RESOURCE_ALIASES['p'], 'products');
      assert.strictEqual(RESOURCE_ALIASES['i'], 'inventory');
      assert.strictEqual(RESOURCE_ALIASES['r'], 'returns');
    });

    it('should have abbreviated aliases', () => {
      assert.strictEqual(RESOURCE_ALIASES['cust'], 'customers');
      assert.strictEqual(RESOURCE_ALIASES['ord'], 'orders');
      assert.strictEqual(RESOURCE_ALIASES['prod'], 'products');
      assert.strictEqual(RESOURCE_ALIASES['inv'], 'inventory');
      assert.strictEqual(RESOURCE_ALIASES['ret'], 'returns');
    });

    it('should have special aliases', () => {
      assert.strictEqual(RESOURCE_ALIASES['stock'], 'inventory');
    });
  });

  describe('action aliases', () => {
    it('should have list aliases', () => {
      assert.strictEqual(ACTION_ALIASES['l'], 'list');
      assert.strictEqual(ACTION_ALIASES['ls'], 'list');
    });

    it('should have action shortcuts', () => {
      assert.strictEqual(ACTION_ALIASES['g'], 'get');
      assert.strictEqual(ACTION_ALIASES['s'], 'ship');
      assert.strictEqual(ACTION_ALIASES['x'], 'cancel');
      assert.strictEqual(ACTION_ALIASES['a'], 'adjust');
    });

    it('should have count shortcuts', () => {
      assert.strictEqual(ACTION_ALIASES['n'], 'count');
      assert.strictEqual(ACTION_ALIASES['#'], 'count');
    });
  });

  describe('expandResource', () => {
    it('should expand single letter aliases', () => {
      assert.strictEqual(expandResource('c'), 'customers');
      assert.strictEqual(expandResource('o'), 'orders');
    });

    it('should expand abbreviated aliases', () => {
      assert.strictEqual(expandResource('cust'), 'customers');
      assert.strictEqual(expandResource('inv'), 'inventory');
    });

    it('should be case insensitive', () => {
      assert.strictEqual(expandResource('C'), 'customers');
      assert.strictEqual(expandResource('ORDERS'), 'orders');
    });

    it('should pass through unknown resources', () => {
      assert.strictEqual(expandResource('unknown'), 'unknown');
    });

    it('should handle null/undefined', () => {
      assert.strictEqual(expandResource(null), null);
      assert.strictEqual(expandResource(undefined), undefined);
    });
  });

  describe('expandAction', () => {
    it('should expand action aliases', () => {
      assert.strictEqual(expandAction('l'), 'list');
      assert.strictEqual(expandAction('g'), 'get');
      assert.strictEqual(expandAction('s'), 'ship');
    });

    it('should be case insensitive', () => {
      assert.strictEqual(expandAction('L'), 'list');
      assert.strictEqual(expandAction('LS'), 'list');
    });

    it('should pass through unknown actions', () => {
      assert.strictEqual(expandAction('create'), 'create');
    });
  });

  describe('getCommand', () => {
    it('should get command by full name', () => {
      const cmd = getCommand('customers');
      assert.ok(cmd);
      assert.strictEqual(cmd.metadata.name, 'customers');
    });

    it('should get command by alias', () => {
      const cmd = getCommand('c');
      assert.ok(cmd);
      assert.strictEqual(cmd.metadata.name, 'customers');
    });

    it('should return undefined for unknown command', () => {
      const cmd = getCommand('unknown');
      assert.strictEqual(cmd, undefined);
    });
  });

  describe('command metadata', () => {
    describe('customers', () => {
      const meta = commands.customers.metadata;

      it('should have correct name', () => {
        assert.strictEqual(meta.name, 'customers');
      });

      it('should have aliases', () => {
        assert.ok(meta.aliases.includes('c'));
        assert.ok(meta.aliases.includes('cust'));
      });

      it('should have all actions', () => {
        assert.ok(meta.actions.list);
        assert.ok(meta.actions.get);
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.count);
      });
    });

    describe('orders', () => {
      const meta = commands.orders.metadata;

      it('should have order-specific actions', () => {
        assert.ok(meta.actions.ship);
        assert.ok(meta.actions.cancel);
        assert.ok(meta.actions.status);
        assert.ok(meta.actions.pending);
        assert.ok(meta.actions.recent);
      });
    });

    describe('inventory', () => {
      const meta = commands.inventory.metadata;

      it('should have inventory-specific actions', () => {
        assert.ok(meta.actions.stock);
        assert.ok(meta.actions.adjust);
        assert.ok(meta.actions.low);
        assert.ok(meta.actions.reserve);
        assert.ok(meta.actions.release);
      });
    });

    describe('returns', () => {
      const meta = commands.returns.metadata;

      it('should have return-specific actions', () => {
        assert.ok(meta.actions.approve);
        assert.ok(meta.actions.reject);
        assert.ok(meta.actions.pending);
        assert.ok(meta.actions.stats);
      });
    });
  });

  describe('error handling', () => {
    it('should throw descriptive errors for unknown actions', async () => {
      const mockContext = {
        commerce: {},
        output: { table: () => '' },
        jsonOutput: false,
        resolveId: async (id) => id
      };

      try {
        await commands.customers.execute('unknown_action', [], mockContext);
        assert.fail('Should have thrown');
      } catch (error) {
        assert.ok(error.message.includes('Unknown action'));
        assert.ok(error.message.includes('Available actions'));
      }
    });

    it('should throw descriptive errors for missing arguments', async () => {
      const mockContext = {
        commerce: {
          customers: {
            get: async () => null,
            getByEmail: async () => null
          }
        },
        output: { table: () => '' },
        jsonOutput: false,
        resolveId: async (id) => id
      };

      try {
        await commands.customers.execute('get', [], mockContext);
        assert.fail('Should have thrown');
      } catch (error) {
        assert.ok(error.message.includes('Usage'));
      }
    });
  });
});
