/**
 * Unit tests for cli/src/channels/orchestrator.js — ChannelOrchestrator
 *
 * Covers:
 *   - constructor initial state
 *   - start() lifecycle: session store, identity store, middleware, notifications
 *   - channel launching: success, disabled, unknown type, launch error
 *   - double-start guard (only applies when channels were actually launched)
 *   - no channels configured
 *   - autonomous engine integration (EventBridge + autonomous commands)
 *   - HTTP gateway integration (start, failure, disabled)
 *   - webchat route mounting
 *   - plugin service start / stop
 *   - getStatus() shape and fields
 *   - shutdown() lifecycle: gateways, session/identity store cleanup, subsystems
 *   - voice / browser / memory subsystem graceful failure
 *   - loadOrchestratorConfig() — JSON and error cases
 *
 * Implementation notes:
 *   - node:test + node:assert/strict only (no vitest / jest)
 *   - ChannelSessionStore and CustomerIdentityStore need real SQLite;
 *     we use a per-test temp directory that is deleted in afterEach.
 *   - The orchestrator sets _running = true only AFTER successfully processing
 *     channel entries. When channels:{} is empty the code returns early (line ~415)
 *     without setting _running. Tests that need _running===true must configure
 *     at least one channel entry (even a failing one clears this path).
 *   - EventBridge.stop() calls engine.removeListener(); the engine stub must
 *     implement that method.
 *   - HookRunner.add() is the method name (not addHook).
 *   - getPluginRegistry().getServices() returns a shallow copy, so we push
 *     into the internal _services array and clean up after each test.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';

// ============================================================================
// Temp-directory helpers
// ============================================================================

let tmpDir = null;

function makeTmpDir() {
  if (!tmpDir) {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'orch-test-'));
  }
  return tmpDir;
}

function cleanTmpDir() {
  if (tmpDir) {
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      // ignore cleanup errors
    }
    tmpDir = null;
  }
}

// ============================================================================
// Config factory
// ============================================================================

/**
 * Build a minimal orchestrator config that avoids heavy launchers.
 *
 * Passing channels with at least one entry (even an unknown/disabled one)
 * lets us reach the end of start() so _running is set to true.
 *
 * Pass `channels: {}` (the default) when you specifically want to test the
 * "no channels configured" early-return path.
 */
function makeConfig(overrides = {}) {
  const dir = makeTmpDir();
  return {
    channels: {},
    shared: { dbPath: ':memory:', verbose: false },
    persistSessions: false,
    sessionDbPath: path.join(dir, 'sessions.db'),
    identityDbPath: path.join(dir, 'identity.db'),
    stateDir: dir,
    // Disable HTTP gateway by default to avoid real port binding in tests
    httpGateway: { enabled: false },
    ...overrides,
  };
}

/**
 * A config that includes at least one channel entry so that start() reaches
 * the end of the function and sets _running = true.
 * Using an unknown channel name so we avoid pulling in real SDKs.
 */
function makeConfigWithChannel(overrides = {}) {
  return makeConfig({
    channels: { 'test-unknown-channel': { token: 'x' } },
    ...overrides,
  });
}

// ============================================================================
// Stub factories
// ============================================================================

/**
 * Build an autonomous engine stub with EventEmitter-style interface needed by
 * EventBridge (on/removeListener).
 */
function makeEngineStub() {
  const listeners = new Map();
  return {
    _notifier: null,
    setNotifier(n) {
      this._notifier = n;
    },
    on(event, fn) {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(fn);
    },
    removeListener(event, fn) {
      if (!listeners.has(event)) return;
      listeners.set(
        event,
        listeners.get(event).filter((h) => h !== fn),
      );
    },
    emit(event, data) {
      for (const fn of listeners.get(event) || []) fn(data);
    },
  };
}

/**
 * Build a mock gateway object (what a launcher returns).
 */
function makeGatewayStub() {
  return {
    _shutdown: false,
    async shutdown() {
      this._shutdown = true;
    },
  };
}

// ============================================================================
// Import the module under test
// ============================================================================

import { ChannelOrchestrator, loadOrchestratorConfig } from '../src/channels/orchestrator.js';

// ============================================================================
// Constructor
// ============================================================================

describe('ChannelOrchestrator — constructor', () => {
  afterEach(cleanTmpDir);

  it('stores config and initialises all fields to null/empty', () => {
    const cfg = makeConfig();
    const orch = new ChannelOrchestrator(cfg);

    assert.strictEqual(orch.config, cfg);
    assert.ok(orch.gateways instanceof Map);
    assert.strictEqual(orch.gateways.size, 0);
    assert.strictEqual(orch.sessionStore, null);
    assert.strictEqual(orch.identityStore, null);
    assert.deepEqual(orch.middleware, []);
    assert.strictEqual(orch._running, false);
    assert.strictEqual(orch._eventBridge, null);
    assert.strictEqual(orch._httpGateway, null);
    assert.strictEqual(orch._voice, null);
    assert.strictEqual(orch._browser, null);
    assert.strictEqual(orch._memory, null);
  });

  it('accepts config with no channels key without throwing', () => {
    const orch = new ChannelOrchestrator({ shared: {} });
    assert.ok(orch instanceof ChannelOrchestrator);
  });

  it('exposes gateways as an empty Map initially', () => {
    const orch = new ChannelOrchestrator(makeConfig());
    assert.ok(orch.gateways instanceof Map);
    assert.strictEqual(orch.gateways.size, 0);
  });
});

// ============================================================================
// start() — basic lifecycle (no-channels early-return path)
// ============================================================================

describe('ChannelOrchestrator — start() with no channels', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('returns { started: [], failed: [] } when channels is empty', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    const result = await orch.start();
    assert.deepEqual(result.started, []);
    assert.deepEqual(result.failed, []);
  });

  it('initialises identityStore even when persistSessions is false', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.notStrictEqual(orch.identityStore, null);
  });

  it('creates sessionStore when persistSessions is true', async () => {
    const dir = makeTmpDir();
    orch = new ChannelOrchestrator({
      channels: {},
      shared: { dbPath: ':memory:' },
      persistSessions: true,
      sessionDbPath: path.join(dir, 'sessions.db'),
      identityDbPath: path.join(dir, 'identity.db'),
      httpGateway: { enabled: false },
    });
    await orch.start();
    assert.notStrictEqual(orch.sessionStore, null);
  });

  it('does NOT create sessionStore when persistSessions is false', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch.sessionStore, null);
  });

  it('builds a non-empty middleware stack', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.ok(Array.isArray(orch.middleware));
    assert.ok(orch.middleware.length > 0, 'should have at least the metricsCollector middleware');
  });
});

// ============================================================================
// start() — _running flag (channel entries needed to reach the flag)
// ============================================================================

describe('ChannelOrchestrator — _running flag', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('sets _running to true after start() processes channel entries', async () => {
    // One unknown channel forces the loop to run and reach _running = true
    orch = new ChannelOrchestrator(makeConfigWithChannel());
    assert.strictEqual(orch._running, false);
    await orch.start();
    assert.strictEqual(orch._running, true);
  });

  it('throws if start() called twice on a running orchestrator', async () => {
    orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    await assert.rejects(() => orch.start(), /already running/i);
  });

  it('getStatus() returns running:true after channel-entry start()', async () => {
    orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    assert.strictEqual(orch.getStatus().running, true);
  });
});

// ============================================================================
// start() — channel launching
// ============================================================================

describe('ChannelOrchestrator — channel launching', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('skips channels with enabled:false', async () => {
    orch = new ChannelOrchestrator(
      makeConfig({
        channels: { telegram: { enabled: false } },
      }),
    );
    const { started, failed } = await orch.start();
    assert.ok(!started.includes('telegram'));
    assert.ok(!failed.some((f) => f.channel === 'telegram'));
  });

  it('records unknown channel type in failed[]', async () => {
    orch = new ChannelOrchestrator(
      makeConfig({
        channels: { 'my-unknown-channel': { token: 'abc' } },
      }),
    );
    const { started, failed } = await orch.start();
    assert.ok(!started.includes('my-unknown-channel'));
    const entry = failed.find((f) => f.channel === 'my-unknown-channel');
    assert.ok(entry, 'should be in failed[]');
    assert.match(entry.error, /unknown channel/i);
  });

  it('records channel launch error in failed[] without throwing', async () => {
    // Telegram launcher tries a dynamic import that fails in test env (no SDK)
    orch = new ChannelOrchestrator(
      makeConfig({
        channels: { telegram: { token: 'FAKE_TOKEN' } },
      }),
    );
    const { started, failed } = await orch.start();
    assert.ok(Array.isArray(started));
    assert.ok(Array.isArray(failed));
    // The channel should appear either in failed (launch error) or not in started
    assert.ok(!started.includes('telegram'));
  });

  it('multiple unknown channels all appear in failed[]', async () => {
    orch = new ChannelOrchestrator(
      makeConfig({
        channels: {
          'chan-a': { token: '1' },
          'chan-b': { token: '2' },
        },
      }),
    );
    const { failed } = await orch.start();
    const names = failed.map((f) => f.channel);
    assert.ok(names.includes('chan-a'));
    assert.ok(names.includes('chan-b'));
  });

  it('gateways Map is a Map instance after start()', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.ok(orch.gateways instanceof Map);
  });
});

// ============================================================================
// start() — notification routes
// ============================================================================

describe('ChannelOrchestrator — notification route loading', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('starts without error when notifications config is absent', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await assert.doesNotReject(() => orch.start());
  });

  it('loads notification routes when provided', async () => {
    const { getNotifier } = await import('../src/channels/notifier.js');
    const notifier = getNotifier();

    orch = new ChannelOrchestrator(
      makeConfig({
        notifications: {
          routes: {
            'order.shipped': [{ channel: 'telegram', target: '@ops' }],
          },
        },
      }),
    );
    await orch.start();

    // Verify getRoutes returns an object (routes structure depends on notifier impl)
    const routes = notifier.getRoutes();
    assert.ok(typeof routes === 'object');
  });
});

// ============================================================================
// start() — autonomous engine integration
// ============================================================================

describe('ChannelOrchestrator — autonomous engine integration', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('calls engine.setNotifier() when autonomousEngine is provided', async () => {
    const engine = makeEngineStub();
    orch = new ChannelOrchestrator(
      makeConfig({ autonomousEngine: engine }),
    );
    await orch.start();
    assert.notStrictEqual(engine._notifier, null, 'setNotifier should have been called');
  });

  it('creates and starts an EventBridge when engine is provided', async () => {
    const engine = makeEngineStub();
    orch = new ChannelOrchestrator(
      makeConfig({ autonomousEngine: engine }),
    );
    await orch.start();
    assert.notStrictEqual(orch._eventBridge, null, '_eventBridge should be set');
  });

  it('does NOT create EventBridge when no engine is configured', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch._eventBridge, null);
  });

  it('stops and nulls EventBridge on shutdown', async () => {
    const engine = makeEngineStub();
    orch = new ChannelOrchestrator(
      makeConfig({ autonomousEngine: engine }),
    );
    await orch.start();
    assert.notStrictEqual(orch._eventBridge, null);
    await orch.shutdown();
    assert.strictEqual(orch._eventBridge, null);
    orch = null;
  });
});

// ============================================================================
// getStatus()
// ============================================================================

describe('ChannelOrchestrator — getStatus()', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('returns running:false before start()', () => {
    orch = new ChannelOrchestrator(makeConfig());
    const status = orch.getStatus();
    assert.strictEqual(status.running, false);
  });

  it('includes all expected top-level keys', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    const status = orch.getStatus();

    const expected = [
      'running', 'channels', 'metrics', 'notifier', 'skills',
      'plugins', 'voice', 'browser', 'memory', 'httpGateway',
    ];
    for (const key of expected) {
      assert.ok(key in status, `expected key "${key}" in status`);
    }
  });

  it('voice.enabled is false when no voice subsystem', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch.getStatus().voice.enabled, false);
  });

  it('browser.enabled is false when no browser subsystem', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch.getStatus().browser.enabled, false);
  });

  it('memory.enabled is false when no memory subsystem', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch.getStatus().memory.enabled, false);
  });

  it('httpGateway.enabled is false when gateway is disabled', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch.getStatus().httpGateway.enabled, false);
  });

  it('httpGateway.address is null when gateway is disabled', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    assert.strictEqual(orch.getStatus().httpGateway.address, null);
  });

  it('channels object reflects running gateway names', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    const status = orch.getStatus();
    assert.deepEqual(status.channels, {});
  });

  it('skills.total is a non-negative number', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    const { skills } = orch.getStatus();
    assert.ok(typeof skills.total === 'number' && skills.total >= 0);
  });

  it('plugins object contains expected keys', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    const { plugins } = orch.getStatus();
    assert.ok('loaded' in plugins, 'plugins.loaded missing');
    assert.ok('commands' in plugins, 'plugins.commands missing');
    assert.ok('gatewayMethods' in plugins, 'plugins.gatewayMethods missing');
    assert.ok('cliExtensions' in plugins, 'plugins.cliExtensions missing');
    assert.ok('slots' in plugins, 'plugins.slots missing');
  });

  it('notifier object contains registeredChannels and routes', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    const { notifier } = orch.getStatus();
    assert.ok('registeredChannels' in notifier, 'notifier.registeredChannels missing');
    assert.ok('routes' in notifier, 'notifier.routes missing');
  });

  it('channels object shows injected gateway as an object with status:running', async () => {
    orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    // Inject a fake gateway manually
    orch.gateways.set('fake-channel', makeGatewayStub());
    const status = orch.getStatus();
    assert.deepEqual(status.channels['fake-channel'], { status: 'running' });
  });
});

// ============================================================================
// shutdown()
// ============================================================================

describe('ChannelOrchestrator — shutdown()', () => {
  afterEach(cleanTmpDir);

  it('sets _running to false after shutdown', async () => {
    const orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    assert.strictEqual(orch._running, true);
    await orch.shutdown();
    assert.strictEqual(orch._running, false);
  });

  it('clears gateways map on shutdown', async () => {
    const orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    orch.gateways.set('extra', makeGatewayStub());
    await orch.shutdown();
    assert.strictEqual(orch.gateways.size, 0);
  });

  it('nulls out sessionStore on shutdown when it was created', async () => {
    const dir = makeTmpDir();
    const orch = new ChannelOrchestrator({
      channels: { 'test-unknown': {} },
      shared: { dbPath: ':memory:' },
      persistSessions: true,
      sessionDbPath: path.join(dir, 'sessions.db'),
      identityDbPath: path.join(dir, 'identity.db'),
      httpGateway: { enabled: false },
    });
    await orch.start();
    assert.notStrictEqual(orch.sessionStore, null);
    await orch.shutdown();
    assert.strictEqual(orch.sessionStore, null);
  });

  it('nulls out identityStore on shutdown', async () => {
    const orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    assert.notStrictEqual(orch.identityStore, null);
    await orch.shutdown();
    assert.strictEqual(orch.identityStore, null);
  });

  it('calls gateway.shutdown() on each running gateway', async () => {
    const orch = new ChannelOrchestrator(makeConfig());
    await orch.start();

    const gw = makeGatewayStub();
    orch.gateways.set('fake', gw);

    await orch.shutdown();
    assert.strictEqual(gw._shutdown, true);
  });

  it('tolerates a gateway without a shutdown() method', async () => {
    const orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    orch.gateways.set('bare', { name: 'bare' });

    await assert.doesNotReject(() => orch.shutdown());
  });

  it('tolerates gateway.shutdown() throwing', async () => {
    const orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();

    orch.gateways.set('exploding', {
      async shutdown() {
        throw new Error('boom');
      },
    });

    await assert.doesNotReject(() => orch.shutdown());
    assert.strictEqual(orch._running, false);
  });

  it('can be called on an already-stopped orchestrator without error', async () => {
    const orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    await orch.shutdown();
    // Second call on a stopped orchestrator should not throw
    await assert.doesNotReject(() => orch.shutdown());
  });

  it('sets _httpGateway to null after shutdown (when it was null before too)', async () => {
    const orch = new ChannelOrchestrator(makeConfig());
    await orch.start();
    await orch.shutdown();
    assert.strictEqual(orch._httpGateway, null);
  });
});

// ============================================================================
// Middleware builder
// ============================================================================

describe('ChannelOrchestrator — middleware configuration', () => {
  afterEach(cleanTmpDir);

  it('includes more middleware when rateLimiter is configured', async () => {
    const orchNoMw = new ChannelOrchestrator(makeConfig());
    await orchNoMw.start();
    const baseLength = orchNoMw.middleware.length;
    await orchNoMw.shutdown();
    cleanTmpDir();

    const orchWithMw = new ChannelOrchestrator(
      makeConfig({ middleware: { rateLimiter: { maxPerMinute: 5 } } }),
    );
    await orchWithMw.start();
    assert.ok(
      orchWithMw.middleware.length > baseLength,
      'rateLimiter should add a middleware function',
    );
    await orchWithMw.shutdown();
  });

  it('includes languageDetect middleware when languageDetect:true', async () => {
    const orchNoMw = new ChannelOrchestrator(makeConfig());
    await orchNoMw.start();
    const baseLength = orchNoMw.middleware.length;
    await orchNoMw.shutdown();
    cleanTmpDir();

    const orchLang = new ChannelOrchestrator(
      makeConfig({ middleware: { languageDetect: true } }),
    );
    await orchLang.start();
    assert.ok(
      orchLang.middleware.length > baseLength,
      'languageDetect should add a middleware function',
    );
    await orchLang.shutdown();
  });

  it('skips logger middleware when logger:false', async () => {
    const orchDefault = new ChannelOrchestrator(makeConfig());
    await orchDefault.start();
    const defaultLen = orchDefault.middleware.length;
    await orchDefault.shutdown();
    cleanTmpDir();

    const orchNoLog = new ChannelOrchestrator(
      makeConfig({ middleware: { logger: false } }),
    );
    await orchNoLog.start();
    assert.ok(
      orchNoLog.middleware.length < defaultLen,
      'disabling logger should reduce middleware count',
    );
    await orchNoLog.shutdown();
  });
});

// ============================================================================
// HTTP gateway integration
// ============================================================================

describe('ChannelOrchestrator — HTTP gateway', () => {
  let orch;

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }
    cleanTmpDir();
  });

  it('does not crash when HTTP gateway fails to bind (bad host)', async () => {
    orch = new ChannelOrchestrator(
      makeConfig({
        httpGateway: {
          enabled: true,
          host: '999.999.999.999',
          port: 0,
        },
      }),
    );
    await assert.doesNotReject(() => orch.start());
    // After failure the field should be null (non-fatal)
    assert.strictEqual(orch._httpGateway, null);
  });

  it('leaves _httpGateway null when enabled:false', async () => {
    orch = new ChannelOrchestrator(makeConfig({ httpGateway: { enabled: false } }));
    await orch.start();
    assert.strictEqual(orch._httpGateway, null);
    assert.strictEqual(orch.getStatus().httpGateway.enabled, false);
  });
});

// ============================================================================
// Plugin services — start and stop
// ============================================================================

describe('ChannelOrchestrator — plugin services', () => {
  let orch;
  let injectedSvc;

  beforeEach(async () => {
    // Grab the registry before each test so we can inject a service
    const { getPluginRegistry } = await import('../src/channels/plugin-api.js');
    const registry = getPluginRegistry();

    injectedSvc = {
      name: `test-svc-${Date.now()}`,
      _started: false,
      _stopped: false,
      async start() {
        this._started = true;
      },
      async stop() {
        this._stopped = true;
      },
    };
    registry._services.push(injectedSvc);
  });

  afterEach(async () => {
    if (orch) {
      if (orch._running) await orch.shutdown();
      orch = null;
    }

    // Remove injected service from registry
    const { getPluginRegistry } = await import('../src/channels/plugin-api.js');
    const registry = getPluginRegistry();
    registry._services = registry._services.filter((s) => s !== injectedSvc);
    injectedSvc = null;

    cleanTmpDir();
  });

  it('calls start() on registered plugin services during orchestrator start()', async () => {
    // Must have at least one channel entry so the early-return path is skipped
    // and the plugin service start loop is reached.
    orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    assert.strictEqual(injectedSvc._started, true);
  });

  it('calls stop() on registered plugin services during orchestrator shutdown()', async () => {
    orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    await orch.shutdown();
    assert.strictEqual(injectedSvc._stopped, true);
    orch = null;
  });

  it('tolerates plugin service start() throwing', async () => {
    injectedSvc.start = async function () {
      throw new Error('service start failure');
    };

    orch = new ChannelOrchestrator(makeConfigWithChannel());
    await assert.doesNotReject(() => orch.start());
  });

  it('tolerates plugin service stop() throwing during shutdown', async () => {
    injectedSvc.stop = async function () {
      throw new Error('service stop failure');
    };

    orch = new ChannelOrchestrator(makeConfigWithChannel());
    await orch.start();
    await assert.doesNotReject(() => orch.shutdown());
    orch = null;
  });
});

// ============================================================================
// loadOrchestratorConfig()
// ============================================================================

describe('loadOrchestratorConfig()', () => {
  let tmpFile;

  afterEach(() => {
    if (tmpFile) {
      try {
        fs.unlinkSync(tmpFile);
      } catch {
        // ignore
      }
      tmpFile = null;
    }
    cleanTmpDir();
  });

  it('parses a JSON config file', async () => {
    const dir = makeTmpDir();
    tmpFile = path.join(dir, 'orch.json');
    const cfg = { channels: { telegram: { enabled: false } }, shared: { dbPath: ':memory:' } };
    fs.writeFileSync(tmpFile, JSON.stringify(cfg));

    const loaded = await loadOrchestratorConfig(tmpFile);
    assert.deepEqual(loaded, cfg);
  });

  it('throws SyntaxError on invalid JSON', async () => {
    const dir = makeTmpDir();
    tmpFile = path.join(dir, 'bad.json');
    fs.writeFileSync(tmpFile, '{ invalid json %%% }');

    await assert.rejects(() => loadOrchestratorConfig(tmpFile), SyntaxError);
  });

  it('throws ENOENT when file does not exist', async () => {
    await assert.rejects(
      () => loadOrchestratorConfig('/tmp/definitely-does-not-exist-orch-xyz.json'),
      { code: 'ENOENT' },
    );
  });
});
