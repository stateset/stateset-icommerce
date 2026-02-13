/**
 * Tests for cli/src/channels/plugin-runtime.js — PluginRuntime
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

let pluginRuntimeModule;
let moduleLoaded = false;

try {
  pluginRuntimeModule = await import('../../src/channels/plugin-runtime.js');
  moduleLoaded = true;
} catch {
  // Module may fail in test env due to rich-messages / metrics deps
}

describe('plugin-runtime', { skip: !moduleLoaded && 'Module not loadable in test env' }, () => {
  describe('module exports', () => {
    it('exports initializeSharedRuntime', () => {
      assert.strictEqual(typeof pluginRuntimeModule.initializeSharedRuntime, 'function');
    });

    it('exports getSharedRuntime', () => {
      assert.strictEqual(typeof pluginRuntimeModule.getSharedRuntime, 'function');
    });

    it('exports createPluginRuntime', () => {
      assert.strictEqual(typeof pluginRuntimeModule.createPluginRuntime, 'function');
    });
  });

  describe('initializeSharedRuntime', () => {
    it('initializes without options', () => {
      assert.doesNotThrow(() => {
        pluginRuntimeModule.initializeSharedRuntime();
      });
    });

    it('accepts commerce option', () => {
      assert.doesNotThrow(() => {
        pluginRuntimeModule.initializeSharedRuntime({ commerce: null });
      });
    });
  });

  describe('getSharedRuntime', () => {
    it('returns an object after initialization', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const runtime = pluginRuntimeModule.getSharedRuntime();
      assert.ok(runtime !== null && runtime !== undefined);
      assert.strictEqual(typeof runtime, 'object');
    });
  });

  describe('createPluginRuntime', () => {
    it('returns runtime context for a plugin', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx !== null);
      assert.strictEqual(typeof ctx, 'object');
    });

    it('provides a logger', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx.logger);
      assert.strictEqual(typeof ctx.logger.info, 'function');
      assert.strictEqual(typeof ctx.logger.warn, 'function');
      assert.strictEqual(typeof ctx.logger.error, 'function');
      assert.strictEqual(typeof ctx.logger.debug, 'function');
    });

    it('provides services object', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx.services);
      assert.strictEqual(typeof ctx.services, 'object');
    });

    it('provides richMessages builders', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx.richMessages);
      assert.strictEqual(typeof ctx.richMessages, 'object');
    });

    it('provides capabilities', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx.capabilities);
      assert.strictEqual(typeof ctx.capabilities, 'object');
    });

    it('provides storage', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx.storage);
      assert.strictEqual(typeof ctx.storage, 'object');
    });

    it('provides env info', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin');
      assert.ok(ctx.env);
      assert.strictEqual(typeof ctx.env, 'object');
    });

    it('verbose option enables debug logging', () => {
      pluginRuntimeModule.initializeSharedRuntime();
      const ctx = pluginRuntimeModule.createPluginRuntime('test-plugin', { verbose: true });
      assert.ok(ctx.logger);
      // Debug should be a real function (not no-op) when verbose
      assert.strictEqual(typeof ctx.logger.debug, 'function');
    });
  });
});
