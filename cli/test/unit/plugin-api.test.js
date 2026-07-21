import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

import {
  HookRunner,
  PluginAPI,
  PluginRegistry,
  getPluginRegistry,
  resetPluginRegistry,
} from '../../src/channels/plugin-api.js';

// ============================================================================
// HookRunner
// ============================================================================

describe('HookRunner', () => {
  let runner;

  beforeEach(() => {
    runner = new HookRunner();
  });

  describe('add / hasHooks', () => {
    it('starts with no hooks', () => {
      assert.ok(!runner.hasHooks('message_received'));
    });

    it('registers a hook handler', () => {
      runner.add('message_received', () => {}, { priority: 100, pluginId: 'p1' });
      assert.ok(runner.hasHooks('message_received'));
    });

    it('returns false for non-existent hook name', () => {
      assert.ok(!runner.hasHooks('nonexistent'));
    });
  });

  describe('remove', () => {
    it('removes all hooks for a plugin', () => {
      runner.add('message_received', () => {}, { priority: 100, pluginId: 'p1' });
      runner.add('message_received', () => {}, { priority: 100, pluginId: 'p2' });
      runner.remove('p1');
      // p2 should remain
      assert.ok(runner.hasHooks('message_received'));
    });

    it('clears hook list when last plugin removed', () => {
      runner.add('message_sent', () => {}, { priority: 100, pluginId: 'p1' });
      runner.remove('p1');
      // The array still exists but is empty
      assert.equal(runner.hasHooks('message_sent'), false);
    });
  });

  describe('run (parallel hooks)', () => {
    it('runs parallel hooks and returns original data', async () => {
      let called = false;
      runner.add(
        'message_received',
        async () => {
          called = true;
        },
        { priority: 100, pluginId: 'p1' },
      );
      const result = await runner.run('message_received', { text: 'hi' });
      assert.ok(called);
      assert.deepEqual(result, { text: 'hi' });
    });

    it('swallows errors in parallel hooks', async () => {
      runner.add(
        'message_received',
        async () => {
          throw new Error('boom');
        },
        { priority: 100, pluginId: 'p1' },
      );
      // Should not throw
      const result = await runner.run('message_received', { text: 'hi' });
      assert.deepEqual(result, { text: 'hi' });
    });

    it('returns data unmodified when no hooks registered', async () => {
      const result = await runner.run('message_received', { key: 'val' });
      assert.deepEqual(result, { key: 'val' });
    });
  });

  describe('run (sequential hooks)', () => {
    it('passes data through sequential handlers', async () => {
      runner.add('message_sending', async (data) => ({ ...data, modified: true }), {
        priority: 100,
        pluginId: 'p1',
      });
      const result = await runner.run('message_sending', { text: 'hello' });
      assert.equal(result.modified, true);
      assert.equal(result.text, 'hello');
    });

    it('chains modifications across handlers', async () => {
      runner.add('message_sending', async (data) => ({ step: 1 }), {
        priority: 50,
        pluginId: 'p1',
      });
      runner.add('message_sending', async (data) => ({ step: data.step + 1 }), {
        priority: 100,
        pluginId: 'p2',
      });
      const result = await runner.run('message_sending', {});
      assert.equal(result.step, 2);
    });

    it('swallows errors in sequential hooks and continues', async () => {
      runner.add(
        'before_agent_start',
        async () => {
          throw new Error('fail');
        },
        { priority: 50, pluginId: 'p1' },
      );
      runner.add('before_agent_start', async (data) => ({ ok: true }), {
        priority: 100,
        pluginId: 'p2',
      });
      const result = await runner.run('before_agent_start', {});
      assert.equal(result.ok, true);
    });
  });

  describe('priority ordering', () => {
    it('runs lower priority (number) first', async () => {
      const order = [];
      runner.add(
        'message_sending',
        async () => {
          order.push('B');
          return {};
        },
        { priority: 200, pluginId: 'p2' },
      );
      runner.add(
        'message_sending',
        async () => {
          order.push('A');
          return {};
        },
        { priority: 50, pluginId: 'p1' },
      );
      await runner.run('message_sending', {});
      assert.deepEqual(order, ['A', 'B']);
    });
  });

  describe('getHookCounts', () => {
    it('returns counts per hook name', () => {
      runner.add('message_received', () => {}, { priority: 100, pluginId: 'p1' });
      runner.add('message_received', () => {}, { priority: 100, pluginId: 'p2' });
      runner.add('message_sent', () => {}, { priority: 100, pluginId: 'p1' });
      const counts = runner.getHookCounts();
      assert.equal(counts['message_received'], 2);
      assert.equal(counts['message_sent'], 1);
    });

    it('returns empty object when no hooks', () => {
      assert.deepEqual(runner.getHookCounts(), {});
    });
  });

  describe('clear', () => {
    it('removes all hooks', () => {
      runner.add('message_received', () => {}, { priority: 100, pluginId: 'p1' });
      runner.clear();
      assert.ok(!runner.hasHooks('message_received'));
      assert.deepEqual(runner.getHookCounts(), {});
    });
  });

  describe('static hook sets', () => {
    it('PARALLEL_HOOKS includes message_received', () => {
      assert.ok(HookRunner.PARALLEL_HOOKS.has('message_received'));
    });

    it('SEQUENTIAL_HOOKS includes message_sending', () => {
      assert.ok(HookRunner.SEQUENTIAL_HOOKS.has('message_sending'));
    });
  });
});

// ============================================================================
// PluginAPI
// ============================================================================

describe('PluginAPI', () => {
  let registry;
  let api;

  beforeEach(() => {
    registry = new PluginRegistry();
    api = new PluginAPI('test-plugin', registry);
  });

  describe('constructor', () => {
    it('stores pluginId', () => {
      assert.equal(api.getPluginId(), 'test-plugin');
    });

    it('starts with empty commands', () => {
      assert.deepEqual(api.getRegisteredCommands(), []);
    });

    it('starts with empty services', () => {
      assert.deepEqual(api.getRegisteredServices(), []);
    });
  });

  describe('on', () => {
    it('delegates to hookRunner', () => {
      api.on('message_received', () => {});
      assert.ok(registry.getHookRunner().hasHooks('message_received'));
    });

    it('accepts custom priority', () => {
      api.on('message_received', () => {}, { priority: 10 });
      assert.ok(registry.getHookRunner().hasHooks('message_received'));
    });
  });

  describe('registerService', () => {
    it('registers a valid service', () => {
      api.registerService({ name: 'my-svc', start: async () => {}, stop: async () => {} });
      assert.deepEqual(api.getRegisteredServices(), ['my-svc']);
    });

    it('throws for service without required fields', () => {
      assert.throws(() => api.registerService({ name: 'x' }), /start.*stop/i);
    });

    it('pushes service to registry._services', () => {
      api.registerService({ name: 'svc', start: async () => {}, stop: async () => {} });
      assert.equal(registry.getServices().length, 1);
    });
  });

  describe('registerHttpRoute', () => {
    it('registers a valid route', () => {
      api.registerHttpRoute({ method: 'GET', path: '/test', handler: async () => {} });
      assert.equal(registry.getRoutes().length, 1);
    });

    it('throws for missing required fields', () => {
      assert.throws(() => api.registerHttpRoute({ method: 'GET' }), /path.*handler/i);
    });

    it('throws for invalid level', () => {
      assert.throws(
        () =>
          api.registerHttpRoute({
            method: 'GET',
            path: '/x',
            handler: async () => {},
            level: 'superadmin',
          }),
        /Invalid route level/,
      );
    });

    it('accepts valid level values', () => {
      for (const level of ['none', 'read', 'preview', 'write', 'delete', 'admin']) {
        const r = new PluginRegistry();
        const a = new PluginAPI('p', r);
        a.registerHttpRoute({ method: 'GET', path: '/x', handler: async () => {}, level });
        assert.equal(r.getRoutes().length, 1);
      }
    });

    it('normalizes level to lowercase', () => {
      api.registerHttpRoute({ method: 'GET', path: '/x', handler: async () => {}, level: 'ADMIN' });
      assert.equal(registry.getRoutes()[0].level, 'admin');
    });
  });
});

// ============================================================================
// PluginRegistry
// ============================================================================

describe('PluginRegistry', () => {
  let registry;

  beforeEach(() => {
    registry = new PluginRegistry();
  });

  describe('register', () => {
    it('registers a plugin and calls initFn with API', async () => {
      let receivedApi = null;
      const entry = await registry.register('my-plugin', (api) => {
        receivedApi = api;
      });
      assert.ok(receivedApi);
      assert.equal(receivedApi.getPluginId(), 'my-plugin');
      assert.equal(entry.id, 'my-plugin');
    });

    it('throws for duplicate plugin id', async () => {
      await registry.register('dup', () => {});
      await assert.rejects(() => registry.register('dup', () => {}), /already registered/);
    });

    it('wraps initFn errors', async () => {
      await assert.rejects(
        () =>
          registry.register('bad', () => {
            throw new Error('init boom');
          }),
        /Failed to initialize/,
      );
    });

    it('supports async initFn', async () => {
      let called = false;
      await registry.register('async-plugin', async () => {
        called = true;
      });
      assert.ok(called);
    });
  });

  describe('unregister', () => {
    it('removes a registered plugin', async () => {
      await registry.register('p1', () => {});
      const result = await registry.unregister('p1');
      assert.ok(result);
      assert.equal(registry.has('p1'), false);
    });

    it('returns false for non-existent plugin', async () => {
      const result = await registry.unregister('nope');
      assert.equal(result, false);
    });

    it('stops services on unregister', async () => {
      let stopped = false;
      await registry.register('p1', (api) => {
        api.registerService({
          name: 'svc',
          start: async () => {},
          stop: async () => {
            stopped = true;
          },
        });
      });
      await registry.unregister('p1');
      assert.ok(stopped);
    });

    it('removes hooks on unregister', async () => {
      await registry.register('p1', (api) => {
        api.on('message_received', () => {});
      });
      await registry.unregister('p1');
      assert.equal(registry.getHookRunner().hasHooks('message_received'), false);
    });

    it('removes routes on unregister', async () => {
      await registry.register('p1', (api) => {
        api.registerHttpRoute({ method: 'GET', path: '/x', handler: async () => {} });
      });
      await registry.unregister('p1');
      assert.equal(registry.getRoutes().length, 0);
    });
  });

  describe('listPlugins', () => {
    it('lists registered plugins', async () => {
      await registry.register('a', () => {});
      await registry.register('b', () => {});
      const list = registry.listPlugins();
      assert.equal(list.length, 2);
      assert.ok(list.some((p) => p.id === 'a'));
      assert.ok(list.some((p) => p.id === 'b'));
    });

    it('includes command and hook counts', async () => {
      await registry.register('a', (api) => {
        api.on('message_received', () => {});
      });
      const list = registry.listPlugins();
      assert.equal(list[0].hooks, 1);
    });
  });

  describe('has', () => {
    it('returns true for registered plugin', async () => {
      await registry.register('x', () => {});
      assert.ok(registry.has('x'));
    });

    it('returns false for unregistered plugin', () => {
      assert.equal(registry.has('nope'), false);
    });
  });

  describe('clear', () => {
    it('removes all plugins', async () => {
      await registry.register('a', () => {});
      await registry.register('b', () => {});
      await registry.clear();
      assert.deepEqual(registry.listPlugins(), []);
    });
  });
});

// ============================================================================
// Singleton helpers
// ============================================================================

describe('getPluginRegistry / resetPluginRegistry', () => {
  afterEach(async () => {
    await resetPluginRegistry();
  });

  it('returns a singleton', () => {
    const a = getPluginRegistry();
    const b = getPluginRegistry();
    assert.equal(a, b);
  });

  it('resetPluginRegistry clears the singleton', async () => {
    const a = getPluginRegistry();
    await resetPluginRegistry();
    const b = getPluginRegistry();
    assert.notEqual(a, b);
  });
});
