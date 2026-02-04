/**
 * v0.6.0 Heartbeat Monitor Tests
 *
 * Tests for:
 * - Checker functions (pure unit)
 * - HeartbeatMonitor lifecycle, events, enable/disable
 * - HTTP gateway heartbeat routes
 * - Config defaults
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import http from 'http';

// ============================================================================
// Helpers
// ============================================================================

function request(port, method, path, body = null, headers = {}) {
  return new Promise((resolve, reject) => {
    const opts = {
      hostname: '127.0.0.1',
      port,
      path,
      method,
      headers: { ...headers },
    };

    if (body && typeof body === 'object' && !Buffer.isBuffer(body)) {
      body = JSON.stringify(body);
      opts.headers['Content-Type'] = 'application/json';
    }

    const req = http.request(opts, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const raw = Buffer.concat(chunks).toString('utf-8');
        let parsed;
        try { parsed = JSON.parse(raw); } catch { parsed = raw; }
        resolve({ status: res.statusCode, headers: res.headers, body: parsed });
      });
    });

    req.on('error', reject);
    if (body) req.write(typeof body === 'string' ? body : body);
    req.end();
  });
}

/** Create a mock commerce object with configurable responses. */
function mockCommerce(overrides = {}) {
  return {
    analytics: {
      lowStockItems: overrides.lowStockItems || (async () => []),
      salesSummary: overrides.salesSummary || (async () => ({ totalRevenue: 0 })),
    },
    carts: {
      getAbandoned: overrides.getAbandoned || (async () => []),
    },
    returns: {
      list: overrides.returnsList || (async () => []),
    },
    invoices: {
      getOverdue: overrides.getOverdue || (async () => []),
    },
    listSubscriptions: overrides.listSubscriptions || (async () => []),
  };
}

// ============================================================================
// Checker unit tests
// ============================================================================

describe('v0.6.0 — Heartbeat checkers (unit)', () => {
  let BUILTIN_CHECKERS;

  before(async () => {
    const mod = await import('../src/heartbeat/checkers.js');
    BUILTIN_CHECKERS = mod.BUILTIN_CHECKERS;
  });

  it('registry has all 6 checkers', () => {
    assert.ok(BUILTIN_CHECKERS['low-stock']);
    assert.ok(BUILTIN_CHECKERS['abandoned-carts']);
    assert.ok(BUILTIN_CHECKERS['revenue-milestone']);
    assert.ok(BUILTIN_CHECKERS['pending-returns']);
    assert.ok(BUILTIN_CHECKERS['overdue-invoices']);
    assert.ok(BUILTIN_CHECKERS['subscription-churn']);
  });

  // -- low-stock --
  it('low-stock: not triggered when no items', async () => {
    const commerce = mockCommerce();
    const result = await BUILTIN_CHECKERS['low-stock'](commerce, { threshold: 5 });
    assert.equal(result.triggered, false);
  });

  it('low-stock: triggered when items below threshold', async () => {
    const commerce = mockCommerce({
      lowStockItems: async () => [{ sku: 'A', available: 2 }],
    });
    const result = await BUILTIN_CHECKERS['low-stock'](commerce, { threshold: 5 });
    assert.equal(result.triggered, true);
    assert.equal(result.data.items.length, 1);
    assert.ok(result.summary.includes('1 item'));
  });

  it('low-stock: handles commerce error gracefully', async () => {
    const commerce = mockCommerce({
      lowStockItems: async () => { throw new Error('DB down'); },
    });
    const result = await BUILTIN_CHECKERS['low-stock'](commerce, {});
    assert.equal(result.triggered, false);
    assert.ok(result.data.error);
  });

  // -- abandoned-carts --
  it('abandoned-carts: not triggered when no old carts', async () => {
    const commerce = mockCommerce({
      getAbandoned: async () => [{ updatedAt: new Date().toISOString() }],
    });
    const result = await BUILTIN_CHECKERS['abandoned-carts'](commerce, { minAgeHours: 24 });
    assert.equal(result.triggered, false);
  });

  it('abandoned-carts: triggered when old carts exist', async () => {
    const oldDate = new Date(Date.now() - 48 * 3600_000).toISOString();
    const commerce = mockCommerce({
      getAbandoned: async () => [{ updatedAt: oldDate }],
    });
    const result = await BUILTIN_CHECKERS['abandoned-carts'](commerce, { minAgeHours: 24 });
    assert.equal(result.triggered, true);
    assert.equal(result.data.carts.length, 1);
  });

  // -- revenue-milestone --
  it('revenue-milestone: not triggered below target', async () => {
    const commerce = mockCommerce({
      salesSummary: async () => ({ totalRevenue: 500 }),
    });
    const result = await BUILTIN_CHECKERS['revenue-milestone'](commerce, { target: 10000 });
    assert.equal(result.triggered, false);
  });

  it('revenue-milestone: triggered at or above target', async () => {
    const commerce = mockCommerce({
      salesSummary: async () => ({ totalRevenue: 12000 }),
    });
    const result = await BUILTIN_CHECKERS['revenue-milestone'](commerce, { target: 10000 });
    assert.equal(result.triggered, true);
    assert.ok(result.summary.includes('12,000'));
  });

  // -- pending-returns --
  it('pending-returns: not triggered when no old pending returns', async () => {
    const commerce = mockCommerce({
      returnsList: async () => [{ status: 'pending', createdAt: new Date().toISOString() }],
    });
    const result = await BUILTIN_CHECKERS['pending-returns'](commerce, { maxAgeDays: 7 });
    assert.equal(result.triggered, false);
  });

  it('pending-returns: triggered when old pending returns exist', async () => {
    const oldDate = new Date(Date.now() - 14 * 86400_000).toISOString();
    const commerce = mockCommerce({
      returnsList: async () => [{ status: 'pending', createdAt: oldDate }],
    });
    const result = await BUILTIN_CHECKERS['pending-returns'](commerce, { maxAgeDays: 7 });
    assert.equal(result.triggered, true);
  });

  // -- overdue-invoices --
  it('overdue-invoices: not triggered when none overdue', async () => {
    const commerce = mockCommerce();
    const result = await BUILTIN_CHECKERS['overdue-invoices'](commerce, {});
    assert.equal(result.triggered, false);
  });

  it('overdue-invoices: triggered with overdue invoices', async () => {
    const commerce = mockCommerce({
      getOverdue: async () => [{ id: 'inv-1', amountDue: 500 }],
    });
    const result = await BUILTIN_CHECKERS['overdue-invoices'](commerce, {});
    assert.equal(result.triggered, true);
    assert.ok(result.summary.includes('1 overdue'));
  });

  // -- subscription-churn --
  it('subscription-churn: not triggered when no churn', async () => {
    const commerce = mockCommerce();
    const result = await BUILTIN_CHECKERS['subscription-churn'](commerce, {});
    assert.equal(result.triggered, false);
  });

  it('subscription-churn: triggered with cancelled subs', async () => {
    let callCount = 0;
    const commerce = mockCommerce({
      listSubscriptions: async ({ status }) => {
        if (status === 'cancelled') return [{ id: 'sub-1' }];
        return [];
      },
    });
    const result = await BUILTIN_CHECKERS['subscription-churn'](commerce, {});
    assert.equal(result.triggered, true);
    assert.ok(result.summary.includes('1 cancelled'));
  });
});

// ============================================================================
// HeartbeatMonitor lifecycle
// ============================================================================

describe('v0.6.0 — HeartbeatMonitor lifecycle', () => {
  let HeartbeatMonitor;

  before(async () => {
    const mod = await import('../src/heartbeat/heartbeat.js');
    HeartbeatMonitor = mod.HeartbeatMonitor;
  });

  it('constructor sets up checks from defaults', () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });
    const status = hb.getStatus();
    assert.equal(status.running, false);
    assert.equal(status.checkCount, 6);
    assert.equal(status.enabledCount, 0);
  });

  it('constructor accepts custom checks', () => {
    const hb = new HeartbeatMonitor({
      commerce: mockCommerce(),
      checks: [
        { id: 'my-check', name: 'My Check', checker: 'low-stock', intervalMs: 5000, enabled: true, config: { threshold: 5 } },
      ],
    });
    const status = hb.getStatus();
    assert.equal(status.checkCount, 1);
    assert.equal(status.enabledCount, 1);
  });

  it('start/stop lifecycle', () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });
    hb.start();
    assert.equal(hb.getStatus().running, true);
    hb.stop();
    assert.equal(hb.getStatus().running, false);
  });

  it('runCheck returns result for known check', async () => {
    const hb = new HeartbeatMonitor({
      commerce: mockCommerce({
        lowStockItems: async () => [{ sku: 'X', available: 1 }],
      }),
    });
    const result = await hb.runCheck('low-stock');
    assert.ok(result);
    assert.equal(result.triggered, true);
  });

  it('runCheck returns null for unknown check', async () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });
    const result = await hb.runCheck('nonexistent');
    assert.equal(result, null);
  });

  it('emits alert event when check triggers', async () => {
    const hb = new HeartbeatMonitor({
      commerce: mockCommerce({
        lowStockItems: async () => [{ sku: 'Y', available: 0 }],
      }),
    });

    let alertData = null;
    hb.on('alert', (data) => { alertData = data; });

    await hb.runCheck('low-stock');

    assert.ok(alertData);
    assert.equal(alertData.checkId, 'low-stock');
    assert.ok(alertData.summary.includes('1 item'));
  });

  it('emits check:completed after every run', async () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });

    let completed = false;
    hb.on('check:completed', () => { completed = true; });

    await hb.runCheck('low-stock');
    assert.equal(completed, true);
  });

  it('enable/disable toggles check state', () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });
    const check = hb.getCheck('low-stock');
    assert.equal(check.enabled, false);

    hb.enableCheck('low-stock');
    assert.equal(hb.getCheck('low-stock').enabled, true);

    hb.disableCheck('low-stock');
    assert.equal(hb.getCheck('low-stock').enabled, false);
  });

  it('enable/disable returns false for unknown check', () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });
    assert.equal(hb.enableCheck('fake'), false);
    assert.equal(hb.disableCheck('fake'), false);
  });

  it('listChecks returns all checks', () => {
    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });
    const list = hb.listChecks();
    assert.equal(list.length, 6);
    assert.ok(list.every((c) => c.id && c.name));
  });

  it('start schedules enabled checks and runs them', async () => {
    const hb = new HeartbeatMonitor({
      commerce: mockCommerce(),
      checks: [
        { id: 'test-check', name: 'Test', checker: 'low-stock', intervalMs: 60_000, enabled: true, config: {} },
      ],
    });

    hb.start();
    // Give the immediate runCheck() a tick to complete
    await new Promise((r) => setTimeout(r, 50));

    const check = hb.getCheck('test-check');
    assert.ok(check.runCount >= 1);

    hb.stop();
  });

  it('tracks runCount and triggerCount', async () => {
    const hb = new HeartbeatMonitor({
      commerce: mockCommerce({
        lowStockItems: async () => [{ sku: 'Z' }],
      }),
    });

    await hb.runCheck('low-stock');
    await hb.runCheck('low-stock');

    const check = hb.getCheck('low-stock');
    assert.equal(check.runCount, 2);
    assert.equal(check.triggerCount, 2);
  });
});

// ============================================================================
// HTTP Gateway heartbeat routes
// ============================================================================

describe('v0.6.0 — HTTP Gateway heartbeat routes (no subsystem)', () => {
  let gw, port;

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    port = addr.port;
  });

  after(async () => {
    await gw.stop();
  });

  it('GET /heartbeat/status returns 501 when heartbeat not enabled', async () => {
    const res = await request(port, 'GET', '/heartbeat/status');
    assert.equal(res.status, 501);
    assert.ok(res.body.error.toLowerCase().includes('heartbeat'));
  });

  it('GET /heartbeat/checks returns 501 when heartbeat not enabled', async () => {
    const res = await request(port, 'GET', '/heartbeat/checks');
    assert.equal(res.status, 501);
  });

  it('POST /heartbeat/checks/low-stock/run returns 501', async () => {
    const res = await request(port, 'POST', '/heartbeat/checks/low-stock/run');
    assert.equal(res.status, 501);
  });
});

describe('v0.6.0 — HTTP Gateway heartbeat routes (with subsystem)', () => {
  let gw, port;

  before(async () => {
    const { createHttpGateway } = await import('../src/channels/http-gateway.js');
    const { HeartbeatMonitor } = await import('../src/heartbeat/heartbeat.js');

    const hb = new HeartbeatMonitor({ commerce: mockCommerce() });

    gw = createHttpGateway({ port: 0 });
    const addr = await gw.start();
    port = addr.port;

    gw.setSubsystems({ heartbeat: hb });
  });

  after(async () => {
    await gw.stop();
  });

  it('GET /heartbeat/status returns monitor status', async () => {
    const res = await request(port, 'GET', '/heartbeat/status');
    assert.equal(res.status, 200);
    assert.equal(typeof res.body.running, 'boolean');
    assert.equal(typeof res.body.checkCount, 'number');
  });

  it('GET /heartbeat/checks returns all checks', async () => {
    const res = await request(port, 'GET', '/heartbeat/checks');
    assert.equal(res.status, 200);
    assert.ok(Array.isArray(res.body.checks));
    assert.equal(res.body.checks.length, 6);
  });

  it('POST /heartbeat/checks/low-stock/run executes check', async () => {
    const res = await request(port, 'POST', '/heartbeat/checks/low-stock/run');
    assert.equal(res.status, 200);
    assert.equal(typeof res.body.triggered, 'boolean');
    assert.ok(res.body.summary);
  });

  it('POST /heartbeat/checks/low-stock/enable enables check', async () => {
    const res = await request(port, 'POST', '/heartbeat/checks/low-stock/enable');
    assert.equal(res.status, 200);
    assert.equal(res.body.enabled, true);
  });

  it('POST /heartbeat/checks/low-stock/disable disables check', async () => {
    const res = await request(port, 'POST', '/heartbeat/checks/low-stock/disable');
    assert.equal(res.status, 200);
    assert.equal(res.body.enabled, false);
  });

  it('POST /heartbeat/checks/nonexistent/run returns 404', async () => {
    const res = await request(port, 'POST', '/heartbeat/checks/nonexistent/run');
    assert.equal(res.status, 404);
  });
});

// ============================================================================
// Event bridge heartbeat mappings
// ============================================================================

describe('v0.6.0 — EventBridge heartbeat mappings', () => {
  it('DEFAULT_EVENT_MAP includes heartbeat events', async () => {
    // Read the event-bridge source to verify mappings exist
    const { EventBridge } = await import('../src/channels/event-bridge.js');
    const { EventEmitter } = await import('events');

    // Create a mock engine and notifier
    const engine = new EventEmitter();
    const notifications = [];
    const notifier = {
      sendNotification: async (n) => { notifications.push(n); return { sent: 1, errors: 0 }; },
    };

    const bridge = new EventBridge({ engine, notifier });
    bridge.start();

    // Emit a heartbeat alert event
    engine.emit('heartbeat:alert', {
      checkId: 'low-stock',
      checkName: 'Low Stock',
      summary: '3 items low',
    });

    // Give the async handler a tick
    await new Promise((r) => setTimeout(r, 50));

    assert.equal(notifications.length, 1);
    assert.equal(notifications[0].type, 'heartbeat.alert');
    assert.ok(notifications[0].message.includes('Low Stock'));
    assert.ok(notifications[0].message.includes('3 items low'));

    bridge.stop();
  });

  it('heartbeat:check:error routes through EventBridge', async () => {
    const { EventBridge } = await import('../src/channels/event-bridge.js');
    const { EventEmitter } = await import('events');

    const engine = new EventEmitter();
    const notifications = [];
    const notifier = {
      sendNotification: async (n) => { notifications.push(n); return { sent: 1, errors: 0 }; },
    };

    const bridge = new EventBridge({ engine, notifier });
    bridge.start();

    engine.emit('heartbeat:check:error', { checkId: 'low-stock', error: 'DB timeout' });

    await new Promise((r) => setTimeout(r, 50));

    assert.equal(notifications.length, 1);
    assert.equal(notifications[0].type, 'heartbeat.error');
    assert.ok(notifications[0].message.includes('DB timeout'));

    bridge.stop();
  });
});
