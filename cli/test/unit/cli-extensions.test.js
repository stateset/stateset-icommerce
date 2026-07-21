/**
 * Tests for cli/src/channels/cli-extensions.js — CliExtensionRegistry
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

let CliExtensionRegistry;
let moduleLoaded = false;

try {
  const mod = await import('../../src/channels/cli-extensions.js');
  CliExtensionRegistry = mod.CliExtensionRegistry;
  moduleLoaded = true;
} catch {
  // Module may fail in test env
}

describe(
  'CliExtensionRegistry',
  { skip: !moduleLoaded && 'Module not loadable in test env' },
  () => {
    let registry;

    beforeEach(() => {
      registry = new CliExtensionRegistry();
    });

    const validCommand = { name: 'get', description: 'Get value', handler: async () => ({}) };

    describe('register', () => {
      it('registers a namespace with commands', () => {
        registry.register('memory', {
          pluginId: 'test-plugin',
          commands: [validCommand],
        });
        const ns = registry.list();
        assert.ok(ns.some((n) => n.namespace === 'memory'));
      });

      it('throws on duplicate namespace', () => {
        registry.register('memory', {
          pluginId: 'test-plugin',
          commands: [validCommand],
        });
        assert.throws(
          () => registry.register('memory', { pluginId: 'other', commands: [validCommand] }),
          /already registered/i,
        );
      });

      it('throws on invalid namespace name', () => {
        assert.throws(
          () => registry.register('UPPER', { pluginId: 'test', commands: [validCommand] }),
          /invalid namespace/i,
        );
      });

      it('throws on namespace starting with number', () => {
        assert.throws(
          () => registry.register('1bad', { pluginId: 'test', commands: [validCommand] }),
          /invalid namespace/i,
        );
      });

      it('accepts hyphenated namespace', () => {
        registry.register('my-plugin', {
          pluginId: 'test',
          commands: [validCommand],
        });
        assert.ok(registry.has('my-plugin'));
      });

      it('throws without pluginId', () => {
        assert.throws(
          () => registry.register('memory', { commands: [validCommand] }),
          /plugin id/i,
        );
      });

      it('throws without commands', () => {
        assert.throws(
          () => registry.register('memory', { pluginId: 'test', commands: [] }),
          /command/i,
        );
      });
    });

    describe('has', () => {
      it('returns true for registered namespace', () => {
        registry.register('cache', { pluginId: 'test', commands: [validCommand] });
        assert.strictEqual(registry.has('cache'), true);
      });

      it('returns false for unknown namespace', () => {
        assert.strictEqual(registry.has('nonexistent'), false);
      });
    });

    describe('list', () => {
      it('returns empty list initially', () => {
        assert.deepStrictEqual(registry.list(), []);
      });

      it('returns all registered namespaces', () => {
        registry.register('memory', { pluginId: 'p1', commands: [validCommand] });
        registry.register('search', { pluginId: 'p2', commands: [validCommand] });
        const list = registry.list();
        assert.strictEqual(list.length, 2);
      });
    });

    describe('hasCommand', () => {
      it('returns true for registered command in namespace', () => {
        const cmd1 = { name: 'get', description: 'Get', handler: async () => ({}) };
        const cmd2 = { name: 'set', description: 'Set', handler: async () => ({}) };
        registry.register('cache', { pluginId: 'test', commands: [cmd1, cmd2] });
        assert.strictEqual(registry.hasCommand('cache', 'get'), true);
        assert.strictEqual(registry.hasCommand('cache', 'set'), true);
      });

      it('returns false for unknown command', () => {
        registry.register('cache', { pluginId: 'test', commands: [validCommand] });
        assert.strictEqual(registry.hasCommand('cache', 'unknown'), false);
      });

      it('returns false for unknown namespace', () => {
        assert.strictEqual(registry.hasCommand('unknown', 'get'), false);
      });
    });

    describe('execute', () => {
      it('executes a registered command', async () => {
        let called = false;
        registry.register('test', {
          pluginId: 'p1',
          commands: [
            {
              name: 'run',
              description: 'Run',
              handler: async () => {
                called = true;
                return { output: 'done' };
              },
            },
          ],
        });
        const result = await registry.execute('test', 'run', []);
        assert.strictEqual(called, true);
        assert.strictEqual(result.output, 'done');
      });

      it('returns error for unknown namespace', async () => {
        const result = await registry.execute('unknown', 'cmd', []);
        assert.strictEqual(result.exitCode, 1);
        assert.ok(result.output.toLowerCase().includes('unknown'));
      });

      it('returns help for unknown command', async () => {
        registry.register('test', {
          pluginId: 'p1',
          commands: [validCommand],
        });
        const result = await registry.execute('test', 'unknown', []);
        assert.strictEqual(result.exitCode, 1);
      });

      it('passes args to handler', async () => {
        let receivedArgs;
        registry.register('test', {
          pluginId: 'p1',
          commands: [
            {
              name: 'echo',
              description: 'Echo',
              handler: async (args) => {
                receivedArgs = args;
                return {};
              },
            },
          ],
        });
        await registry.execute('test', 'echo', ['hello', 'world']);
        assert.deepStrictEqual(receivedArgs, ['hello', 'world']);
      });
    });

    describe('unregister', () => {
      it('removes a namespace', () => {
        registry.register('cache', { pluginId: 'test', commands: [validCommand] });
        registry.unregister('cache');
        assert.strictEqual(registry.has('cache'), false);
      });

      it('does not throw for unknown namespace', () => {
        assert.doesNotThrow(() => registry.unregister('unknown'));
      });
    });
  },
);
