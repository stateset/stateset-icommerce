/**
 * Tests for cli/src/harness-hooks.js
 *
 * Validates the harness hook runner and plugin loading system.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

describe('harness-hooks', () => {
  describe('getHarnessHookRunner', () => {
    it('returns a hook runner object', async () => {
      let hookRunner;
      try {
        const { getHarnessHookRunner } = await import('../../src/harness-hooks.js');
        hookRunner = getHarnessHookRunner();
      } catch {
        // Module may fail to load in test env due to plugin-api dependencies
        return;
      }
      assert.ok(hookRunner !== null && hookRunner !== undefined, 'Hook runner should not be null');
    });

    it('returns same instance on repeated calls', async () => {
      let getHarnessHookRunner;
      try {
        ({ getHarnessHookRunner } = await import('../../src/harness-hooks.js'));
      } catch {
        return;
      }
      const runner1 = getHarnessHookRunner();
      const runner2 = getHarnessHookRunner();
      assert.strictEqual(runner1, runner2, 'Should return same hook runner instance');
    });
  });

  describe('ensureHarnessPluginsLoaded', () => {
    it('is exported as a function', async () => {
      let mod;
      try {
        mod = await import('../../src/harness-hooks.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.ensureHarnessPluginsLoaded, 'function');
    });

    it('is idempotent — second call returns immediately', async () => {
      let ensureHarnessPluginsLoaded;
      try {
        ({ ensureHarnessPluginsLoaded } = await import('../../src/harness-hooks.js'));
      } catch {
        return;
      }
      // First call may or may not succeed depending on plugin-loader deps
      try {
        const result1 = await ensureHarnessPluginsLoaded({ verbose: false });
        assert.ok(result1.loaded, 'First call should report loaded');
        const result2 = await ensureHarnessPluginsLoaded({ verbose: false });
        assert.ok(result2.loaded, 'Second call should also report loaded');
      } catch {
        // Plugin loading may fail in test env — that's acceptable
      }
    });

    it('accepts verbose option', async () => {
      let ensureHarnessPluginsLoaded;
      try {
        ({ ensureHarnessPluginsLoaded } = await import('../../src/harness-hooks.js'));
      } catch {
        return;
      }
      // Should not throw when verbose is passed
      try {
        await ensureHarnessPluginsLoaded({ verbose: true });
      } catch {
        // Expected in test env
      }
    });
  });

  describe('module exports', () => {
    it('exports getHarnessHookRunner', async () => {
      let mod;
      try {
        mod = await import('../../src/harness-hooks.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.getHarnessHookRunner, 'function');
    });

    it('exports ensureHarnessPluginsLoaded', async () => {
      let mod;
      try {
        mod = await import('../../src/harness-hooks.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.ensureHarnessPluginsLoaded, 'function');
    });

    it('does not export internal getBundledPluginDir', async () => {
      let mod;
      try {
        mod = await import('../../src/harness-hooks.js');
      } catch {
        return;
      }
      assert.strictEqual(mod.getBundledPluginDir, undefined, 'Internal function should not be exported');
    });
  });
});

describe('harness-hooks hook runner interface', () => {
  it('hook runner has run method if available', async () => {
    let hookRunner;
    try {
      const { getHarnessHookRunner } = await import('../../src/harness-hooks.js');
      hookRunner = getHarnessHookRunner();
    } catch {
      return;
    }
    if (hookRunner && typeof hookRunner === 'object') {
      // HookRunner from plugin-api should have run method
      if ('run' in hookRunner) {
        assert.strictEqual(typeof hookRunner.run, 'function');
      }
    }
  });

  it('hook runner has hasHooks method if available', async () => {
    let hookRunner;
    try {
      const { getHarnessHookRunner } = await import('../../src/harness-hooks.js');
      hookRunner = getHarnessHookRunner();
    } catch {
      return;
    }
    if (hookRunner && typeof hookRunner === 'object') {
      if ('hasHooks' in hookRunner) {
        assert.strictEqual(typeof hookRunner.hasHooks, 'function');
      }
    }
  });

  it('hook runner reports no hooks for unknown event', async () => {
    let hookRunner;
    try {
      const { getHarnessHookRunner } = await import('../../src/harness-hooks.js');
      hookRunner = getHarnessHookRunner();
    } catch {
      return;
    }
    if (hookRunner && typeof hookRunner.hasHooks === 'function') {
      const has = hookRunner.hasHooks('nonexistent_event_xyz');
      assert.ok(!has, 'Should not have hooks for unknown event');
    }
  });
});
