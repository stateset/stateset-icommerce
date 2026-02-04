/**
 * Plugin System Tests for StateSet iCommerce v0.6.0
 *
 * Tests: CommandRegistry, HookRunner, PluginRegistry, PluginConfigState,
 *        Manifest validation, ReplyPipeline, Capabilities, and integration.
 *
 * Run: node --test tests/plugin-system.test.js
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  CommandRegistry,
  getCommandRegistry,
  resetCommandRegistry,
} from '../src/channels/command-registry.js';

import {
  HookRunner,
  PluginRegistry,
  PluginAPI,
  getPluginRegistry,
  resetPluginRegistry,
} from '../src/channels/plugin-api.js';

import {
  PluginConfigState,
} from '../src/channels/plugin-config.js';

import {
  validateManifest,
  validateConfig,
  applyConfigDefaults,
} from '../src/channels/plugin-manifest.js';

import {
  ReplyPipeline,
  createReplyPipeline,
} from '../src/channels/reply-pipeline.js';

import {
  getCapabilities,
  registerCapabilities,
  getAllCapabilities,
} from '../src/channels/capabilities.js';

// ============================================================================
// CommandRegistry
// ============================================================================

describe('CommandRegistry', () => {
  let registry;

  beforeEach(() => {
    resetCommandRegistry();
    registry = new CommandRegistry();
  });

  it('should register and retrieve a command', () => {
    registry.register({
      name: 'greet',
      description: 'Say hello',
      handler: async () => ({ response: 'hello' }),
    });

    assert.ok(registry.has('greet'));
    const cmd = registry.get('greet');
    assert.equal(cmd.name, 'greet');
    assert.equal(cmd.description, 'Say hello');
  });

  it('should reject invalid command names', () => {
    assert.throws(() => {
      registry.register({
        name: '123bad',
        description: 'Test',
        handler: async () => ({}),
      });
    }, /Invalid command name/);
  });

  it('should reject reserved command names', () => {
    assert.throws(() => {
      registry.register({
        name: 'help',
        description: 'Override help',
        handler: async () => ({}),
      });
    }, /reserved/);
  });

  it('should reject duplicate command names', () => {
    registry.register({
      name: 'unique',
      description: 'First',
      handler: async () => ({}),
    });

    assert.throws(() => {
      registry.register({
        name: 'unique',
        description: 'Second',
        handler: async () => ({}),
      });
    }, /already registered/);
  });

  it('should support aliases', () => {
    registry.register({
      name: 'greet',
      aliases: ['hi', 'hello'],
      description: 'Greet',
      handler: async () => ({ response: 'hi' }),
    });

    assert.ok(registry.has('hi'));
    assert.ok(registry.has('hello'));
    const cmd = registry.get('hi');
    assert.equal(cmd.name, 'greet');
  });

  it('should support channel-native name resolution', () => {
    registry.register({
      name: 'greet',
      description: 'Greet',
      handler: async () => ({ response: 'hi' }),
      channelNames: { discord: 'say-hi' },
    });

    const cmd = registry.get('say-hi', 'discord');
    assert.ok(cmd);
    assert.equal(cmd.name, 'greet');

    // Should not resolve for wrong channel
    const noMatch = registry.get('say-hi', 'telegram');
    assert.equal(noMatch, null);
  });

  it('should unregister a command and its aliases', () => {
    registry.register({
      name: 'temp',
      aliases: ['t'],
      description: 'Temp',
      handler: async () => ({}),
    });

    assert.ok(registry.has('temp'));
    assert.ok(registry.has('t'));

    const removed = registry.unregister('temp');
    assert.ok(removed);
    assert.ok(!registry.has('temp'));
    assert.ok(!registry.has('t'));
  });

  it('should list all non-hidden commands', () => {
    registry.register({ name: 'visible', description: 'V', handler: async () => ({}) });
    registry.register({ name: 'hidden-cmd', description: 'H', handler: async () => ({}), hidden: true });

    const list = registry.list();
    assert.equal(list.length, 1);
    assert.equal(list[0].name, 'visible');

    const all = registry.list({ includeHidden: true });
    assert.equal(all.length, 2);
  });

  it('should generate help text', () => {
    registry.register({ name: 'foo', description: 'Do foo', handler: async () => ({}) });
    registry.register({ name: 'bar', description: 'Do bar', handler: async () => ({}) });

    const help = registry.generateHelp();
    assert.ok(help.includes('/foo'));
    assert.ok(help.includes('/bar'));
    assert.ok(help.includes('Do foo'));
  });

  it('should lock during execute and unlock after', async () => {
    registry.register({
      name: 'locktest',
      description: 'Test lock',
      handler: async () => ({ response: 'done' }),
    });

    const result = await registry.execute('locktest', '', { channel: 'test' });
    assert.equal(result.response, 'done');
    assert.equal(registry._locked, false);
  });

  it('should reject handler that is not a function', () => {
    assert.throws(() => {
      registry.register({
        name: 'bad',
        description: 'Bad handler',
        handler: 'not a function',
      });
    }, /handler must be a function/);
  });

  it('should resolve canonical name from alias', () => {
    registry.register({
      name: 'primary',
      aliases: ['alt'],
      description: 'Test',
      handler: async () => ({}),
    });

    assert.equal(registry.resolve('alt'), 'primary');
    assert.equal(registry.resolve('primary'), 'primary');
    assert.equal(registry.resolve('nonexistent'), null);
  });
});

// ============================================================================
// HookRunner
// ============================================================================

describe('HookRunner', () => {
  let runner;

  beforeEach(() => {
    runner = new HookRunner();
  });

  it('should run parallel hooks without blocking on errors', async () => {
    const calls = [];

    runner.add('message_received', async () => { calls.push('a'); }, { priority: 100, pluginId: 'p1' });
    runner.add('message_received', async () => { throw new Error('fail'); }, { priority: 100, pluginId: 'p2' });
    runner.add('message_received', async () => { calls.push('b'); }, { priority: 100, pluginId: 'p3' });

    await runner.run('message_received', { text: 'hello' });

    assert.ok(calls.includes('a'));
    assert.ok(calls.includes('b'));
  });

  it('should run sequential hooks in priority order', async () => {
    const order = [];

    runner.add('message_sending', async () => { order.push('low'); }, { priority: 50, pluginId: 'p1' });
    runner.add('message_sending', async () => { order.push('high'); }, { priority: 200, pluginId: 'p2' });
    runner.add('message_sending', async () => { order.push('mid'); }, { priority: 100, pluginId: 'p3' });

    await runner.run('message_sending', { text: 'hello' });

    assert.deepEqual(order, ['low', 'mid', 'high']);
  });

  it('should allow sequential hooks to modify data', async () => {
    runner.add('message_sending', async (data) => {
      return { text: data.text + ' modified' };
    }, { priority: 100, pluginId: 'p1' });

    const result = await runner.run('message_sending', { text: 'original' });
    assert.equal(result.text, 'original modified');
  });

  it('should report hasHooks correctly', () => {
    assert.ok(!runner.hasHooks('message_received'));

    runner.add('message_received', async () => {}, { priority: 100, pluginId: 'p1' });
    assert.ok(runner.hasHooks('message_received'));
  });

  it('should remove hooks by pluginId', () => {
    runner.add('message_received', async () => {}, { priority: 100, pluginId: 'p1' });
    runner.add('message_received', async () => {}, { priority: 100, pluginId: 'p2' });

    runner.remove('p1');

    const counts = runner.getHookCounts();
    assert.equal(counts['message_received'], 1);
  });

  it('should clear all hooks', () => {
    runner.add('message_received', async () => {}, { priority: 100, pluginId: 'p1' });
    runner.add('message_sending', async () => {}, { priority: 100, pluginId: 'p1' });

    runner.clear();
    assert.ok(!runner.hasHooks('message_received'));
    assert.ok(!runner.hasHooks('message_sending'));
  });
});

// ============================================================================
// PluginRegistry
// ============================================================================

describe('PluginRegistry', () => {
  let pluginRegistry;

  beforeEach(async () => {
    resetCommandRegistry();
    await resetPluginRegistry();
    pluginRegistry = new PluginRegistry();
  });

  it('should register a plugin with commands and hooks', async () => {
    await pluginRegistry.register('test-plugin', (api) => {
      api.registerCommand({
        name: 'test-cmd',
        description: 'A test command',
        handler: async () => ({ response: 'test' }),
      });
      api.on('message_received', async () => {}, { priority: 100 });
    });

    assert.ok(pluginRegistry.has('test-plugin'));

    const plugins = pluginRegistry.listPlugins();
    assert.equal(plugins.length, 1);
    assert.equal(plugins[0].id, 'test-plugin');
    assert.ok(plugins[0].commands.includes('test-cmd'));
  });

  it('should reject duplicate plugin registration', async () => {
    await pluginRegistry.register('dup-test', () => {});

    await assert.rejects(
      () => pluginRegistry.register('dup-test', () => {}),
      /already registered/
    );
  });

  it('should unregister and clean up', async () => {
    await pluginRegistry.register('cleanup-test', (api) => {
      api.registerCommand({
        name: 'cleanup-cmd',
        description: 'Cleanup test',
        handler: async () => ({}),
      });
      api.on('message_received', async () => {}, { priority: 100 });
    });

    const removed = await pluginRegistry.unregister('cleanup-test');
    assert.ok(removed);
    assert.ok(!pluginRegistry.has('cleanup-test'));
    assert.equal(pluginRegistry.listPlugins().length, 0);
  });

  it('should return false when unregistering unknown plugin', async () => {
    const removed = await pluginRegistry.unregister('nonexistent');
    assert.equal(removed, false);
  });

  it('should handle services registration', async () => {
    await pluginRegistry.register('svc-test', (api) => {
      api.registerService({
        name: 'test-service',
        start: async () => {},
        stop: async () => {},
      });
    });

    const services = pluginRegistry.getServices();
    assert.equal(services.length, 1);
    assert.equal(services[0].name, 'test-service');
  });

  it('should handle HTTP route registration', async () => {
    await pluginRegistry.register('route-test', (api) => {
      api.registerHttpRoute({
        method: 'GET',
        path: '/api/test',
        handler: async () => ({ body: { ok: true } }),
      });
    });

    const routes = pluginRegistry.getRoutes();
    assert.equal(routes.length, 1);
    assert.equal(routes[0].path, '/api/test');
  });
});

// ============================================================================
// PluginConfigState
// ============================================================================

describe('PluginConfigState', () => {
  it('should resolve enabled by default', () => {
    const state = new PluginConfigState();
    const resolution = state.resolve('any-plugin');
    assert.ok(resolution.enabled);
  });

  it('should respect global switch', () => {
    const state = new PluginConfigState({ globalEnabled: false });
    const resolution = state.resolve('any-plugin');
    assert.ok(!resolution.enabled);
    assert.ok(resolution.reason.includes('globally disabled'));
  });

  it('should respect deny list', () => {
    const state = new PluginConfigState({ deny: ['blocked-plugin'] });
    assert.ok(!state.isEnabled('blocked-plugin'));
    assert.ok(state.isEnabled('allowed-plugin'));
  });

  it('should respect allow list', () => {
    const state = new PluginConfigState({ allow: ['only-this'] });
    assert.ok(state.isEnabled('only-this'));
    assert.ok(!state.isEnabled('not-this'));
  });

  it('should respect per-plugin overrides', () => {
    const state = new PluginConfigState({
      entries: { 'my-plugin': { enabled: false } },
    });
    assert.ok(!state.isEnabled('my-plugin'));
  });

  it('should enable/disable plugins', () => {
    const state = new PluginConfigState();

    state.disable('test');
    assert.ok(!state.isEnabled('test'));

    state.enable('test');
    assert.ok(state.isEnabled('test'));

    state.resetToDefault('test');
    assert.ok(state.isEnabled('test')); // default is enabled
  });

  it('should manage per-plugin config', () => {
    const state = new PluginConfigState();

    state.setConfig('my-plugin', { apiKey: 'secret', timeout: 5000 });
    const config = state.getConfig('my-plugin');
    assert.equal(config.apiKey, 'secret');
    assert.equal(config.timeout, 5000);
  });

  it('should list entries with states', () => {
    const state = new PluginConfigState({
      entries: {
        'enabled-plugin': { enabled: true },
        'disabled-plugin': { enabled: false },
      },
    });

    const entries = state.listEntries();
    assert.equal(entries.length, 2);

    const enabled = entries.find((e) => e.id === 'enabled-plugin');
    assert.ok(enabled.enabled);

    const disabled = entries.find((e) => e.id === 'disabled-plugin');
    assert.ok(!disabled.enabled);
  });

  it('should serialize to JSON', () => {
    const state = new PluginConfigState({
      deny: ['bad'],
      entries: { 'test': { enabled: true, config: { x: 1 } } },
    });

    const json = state.toJSON();
    assert.ok(json.globalEnabled);
    assert.deepEqual(json.deny, ['bad']);
    assert.equal(json.entries.test.enabled, true);
  });

  it('should deny list override per-plugin enable', () => {
    const state = new PluginConfigState({
      deny: ['test'],
      entries: { 'test': { enabled: true } },
    });
    // Deny list wins over per-plugin override
    assert.ok(!state.isEnabled('test'));
  });
});

// ============================================================================
// Plugin Manifest Validation
// ============================================================================

describe('Plugin Manifest', () => {
  it('should validate a complete manifest', () => {
    const result = validateManifest({
      id: 'test-plugin',
      name: 'Test Plugin',
      entry: 'index.js',
      version: '1.0.0',
    });

    assert.ok(result.valid);
    assert.equal(result.manifest.id, 'test-plugin');
    assert.equal(result.manifest.version, '1.0.0');
    assert.equal(result.manifest.kind, 'general');
  });

  it('should reject missing required fields', () => {
    const result = validateManifest({ id: 'test' });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('name')));
    assert.ok(result.errors.some((e) => e.includes('entry')));
  });

  it('should reject invalid ID format', () => {
    const result = validateManifest({
      id: 'INVALID_ID',
      name: 'Test',
      entry: 'index.js',
    });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('Invalid plugin ID')));
  });

  it('should reject invalid kind', () => {
    const result = validateManifest({
      id: 'test',
      name: 'Test',
      entry: 'index.js',
      kind: 'invalid',
    });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('Invalid kind')));
  });

  it('should warn on non-semver version', () => {
    const result = validateManifest({
      id: 'test',
      name: 'Test',
      entry: 'index.js',
      version: 'beta',
    });
    assert.ok(result.valid);
    assert.ok(result.warnings.some((w) => w.includes('SemVer')));
  });

  it('should reject non-object manifest', () => {
    const result = validateManifest(null);
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('JSON object')));
  });
});

describe('Config Validation', () => {
  it('should validate required fields', () => {
    const result = validateConfig({}, {
      required: ['apiKey'],
      properties: { apiKey: { type: 'string' } },
    });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('apiKey')));
  });

  it('should validate property types', () => {
    const result = validateConfig({ count: 'not a number' }, {
      properties: { count: { type: 'number' } },
    });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('expected number')));
  });

  it('should validate enum values', () => {
    const result = validateConfig({ mode: 'invalid' }, {
      properties: { mode: { type: 'string', enum: ['fast', 'slow'] } },
    });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('must be one of')));
  });

  it('should validate min/max for numbers', () => {
    const result = validateConfig({ count: -5 }, {
      properties: { count: { type: 'number', minimum: 0, maximum: 100 } },
    });
    assert.ok(!result.valid);
  });

  it('should pass valid config', () => {
    const result = validateConfig({ apiKey: 'key123', count: 5 }, {
      required: ['apiKey'],
      properties: {
        apiKey: { type: 'string' },
        count: { type: 'number', minimum: 1 },
      },
    });
    assert.ok(result.valid);
  });

  it('should return valid for null schema', () => {
    const result = validateConfig({ anything: true }, null);
    assert.ok(result.valid);
  });
});

describe('Config Defaults', () => {
  it('should merge defaults with user config', () => {
    const result = applyConfigDefaults(
      { apiKey: 'user-key' },
      { apiKey: 'default-key', timeout: 5000 }
    );
    assert.equal(result.apiKey, 'user-key');
    assert.equal(result.timeout, 5000);
  });

  it('should handle null defaults', () => {
    const result = applyConfigDefaults({ x: 1 }, null);
    assert.deepEqual(result, { x: 1 });
  });
});

// ============================================================================
// ReplyPipeline
// ============================================================================

describe('ReplyPipeline', () => {
  it('should send a message directly', async () => {
    const sent = [];
    const pipeline = new ReplyPipeline({
      onBlockReply: async (payload) => { sent.push(payload); },
      dedup: false,
    });

    const result = await pipeline.send({ targetId: 'chat-1', text: 'hello' });
    assert.ok(result.sent);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].text, 'hello');

    await pipeline.shutdown();
  });

  it('should deduplicate messages', async () => {
    const sent = [];
    const pipeline = new ReplyPipeline({
      onBlockReply: async (payload) => { sent.push(payload); },
      dedup: true,
      dedupWindowMs: 5000,
    });

    await pipeline.send({ targetId: 'chat-1', text: 'hello' });
    const dup = await pipeline.send({ targetId: 'chat-1', text: 'hello' });

    assert.ok(!dup.sent);
    assert.equal(dup.reason, 'duplicate');
    assert.equal(sent.length, 1);

    const stats = pipeline.getStats();
    assert.equal(stats.totalDeduped, 1);

    await pipeline.shutdown();
  });

  it('should respect abort signal', async () => {
    const pipeline = new ReplyPipeline({
      onBlockReply: async () => {},
      dedup: false,
    });

    const controller = new AbortController();
    controller.abort();

    const result = await pipeline.send(
      { targetId: 'chat-1', text: 'hello' },
      { signal: controller.signal }
    );

    assert.ok(!result.sent);
    assert.equal(result.reason, 'aborted');

    await pipeline.shutdown();
  });

  it('should buffer and flush messages', async () => {
    const sent = [];
    const pipeline = new ReplyPipeline({
      onBlockReply: async (payload) => { sent.push(payload); },
      bufferMs: 50,
      dedup: false,
    });

    await pipeline.send({ targetId: 'chat-1', text: 'a' });
    await pipeline.send({ targetId: 'chat-1', text: 'b' });

    // Not yet sent
    assert.equal(sent.length, 0);

    // Wait for flush
    await new Promise((resolve) => setTimeout(resolve, 100));

    // Should have flushed
    assert.ok(sent.length > 0);

    await pipeline.shutdown();
  });

  it('should track statistics', async () => {
    const pipeline = new ReplyPipeline({
      onBlockReply: async () => {},
      dedup: false,
    });

    await pipeline.send({ targetId: 'chat-1', text: 'msg1' });
    await pipeline.send({ targetId: 'chat-1', text: 'msg2' });

    const stats = pipeline.getStats();
    assert.equal(stats.totalSent, 2);
    assert.equal(stats.totalErrors, 0);

    await pipeline.shutdown();
  });

  it('should handle sendAll', async () => {
    const sent = [];
    const pipeline = new ReplyPipeline({
      onBlockReply: async (payload) => { sent.push(payload); },
      dedup: false,
    });

    const results = await pipeline.sendAll([
      { targetId: 'chat-1', text: 'a' },
      { targetId: 'chat-1', text: 'b' },
    ]);

    assert.equal(results.length, 2);
    assert.ok(results.every((r) => r.sent));
    assert.equal(sent.length, 2);

    await pipeline.shutdown();
  });

  it('should handle errors in send callback', async () => {
    const pipeline = new ReplyPipeline({
      onBlockReply: async () => { throw new Error('send failed'); },
      dedup: false,
    });

    const result = await pipeline.send({ targetId: 'chat-1', text: 'hello' });
    assert.ok(!result.sent);
    assert.equal(result.reason, 'send failed');

    const stats = pipeline.getStats();
    assert.equal(stats.totalErrors, 1);

    await pipeline.shutdown();
  });

  it('should support streaming sessions', async () => {
    const sent = [];
    const pipeline = new ReplyPipeline({
      onBlockReply: async (payload) => { sent.push(payload); },
      dedup: false,
      coalescing: { enabled: true, flushIntervalMs: 5000, separator: '' },
    });

    const stream = pipeline.startStream('chat-1');
    await stream.write('hello ');
    await stream.write('world');
    const text = await stream.end();

    assert.equal(text, 'hello world');
    assert.ok(sent.length > 0);

    await pipeline.shutdown();
  });
});

// ============================================================================
// Capabilities
// ============================================================================

describe('Capabilities', () => {
  it('should return capabilities for known channels', () => {
    const caps = getCapabilities('telegram');
    assert.ok(caps);
    assert.equal(caps.richMessages, true);
    assert.equal(caps.typing, true);
  });

  it('should return default capabilities for unknown channels', () => {
    const caps = getCapabilities('unknown-channel');
    assert.ok(caps);
    assert.equal(caps.richMessages, false);
  });

  it('should override capabilities', () => {
    registerCapabilities('custom-channel', {
      richMessages: true,
      buttons: true,
      streaming: true,
    });

    const caps = getCapabilities('custom-channel');
    assert.ok(caps.richMessages);
    assert.ok(caps.buttons);
    assert.ok(caps.streaming);
  });

  it('should list all capabilities', () => {
    const all = getAllCapabilities();
    assert.ok(all.telegram);
    assert.ok(all.discord);
    assert.ok(all.slack);
  });
});

// ============================================================================
// Integration Test
// ============================================================================

describe('Integration: Plugin + Command + Hook', () => {
  let registry;
  let pluginRegistry;

  beforeEach(async () => {
    resetCommandRegistry();
    await resetPluginRegistry();
    registry = getCommandRegistry();
    pluginRegistry = getPluginRegistry();
  });

  it('should register a plugin that adds a command and a hook, both callable', async () => {
    const hookCalls = [];

    await pluginRegistry.register('integration-test', (api) => {
      api.registerCommand({
        name: 'ping',
        description: 'Ping-pong',
        handler: async () => ({ response: 'pong' }),
      });

      api.on('message_received', async (data) => {
        hookCalls.push(data);
      }, { priority: 100 });
    });

    // Command should be in the global registry
    assert.ok(registry.has('ping'));
    const cmd = registry.get('ping');
    const result = await cmd.handler('', {});
    assert.equal(result.response, 'pong');

    // Hook should fire
    const hookRunner = pluginRegistry.getHookRunner();
    await hookRunner.run('message_received', { text: 'hello', senderId: 'user-1' });
    assert.equal(hookCalls.length, 1);
    assert.equal(hookCalls[0].text, 'hello');
  });

  it('should clean up commands and hooks on unregister', async () => {
    await pluginRegistry.register('cleanup-int', (api) => {
      api.registerCommand({
        name: 'temp-cmd',
        description: 'Temp',
        handler: async () => ({}),
      });
      api.on('message_received', async () => {}, { priority: 100 });
    });

    assert.ok(registry.has('temp-cmd'));

    await pluginRegistry.unregister('cleanup-int');
    assert.ok(!registry.has('temp-cmd'));
  });
});
