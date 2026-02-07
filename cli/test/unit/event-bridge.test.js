/**
 * Unit tests for channels/event-bridge.js — EventBridge
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { EventEmitter } from 'node:events';
import { EventBridge } from '../../src/channels/event-bridge.js';

// ---------------------------------------------------------------------------
// Helpers — mock notifier and engine
// ---------------------------------------------------------------------------

function mockNotifier() {
  const sent = [];
  return {
    sendNotification: async (notification) => {
      sent.push(notification);
      return { sent: 1, errors: 0 };
    },
    sent,
  };
}

function mockEngine() {
  return new EventEmitter();
}

// ===========================================================================
// EventBridge constructor
// ===========================================================================

describe('EventBridge', () => {
  it('creates with notifier and engine', () => {
    const bridge = new EventBridge({ engine: mockEngine(), notifier: mockNotifier() });
    assert.ok(bridge);
  });

  it('merges custom event map', () => {
    const notifier = mockNotifier();
    const bridge = new EventBridge({
      engine: mockEngine(),
      notifier,
      eventMap: {
        'custom:event': {
          notificationType: 'custom.event',
          message: (d) => `Custom: ${d.msg}`,
        },
      },
    });
    assert.ok(bridge._eventMap['custom:event']);
    // Defaults still present
    assert.ok(bridge._eventMap['heartbeat:alert']);
  });
});

// ===========================================================================
// start / stop
// ===========================================================================

describe('EventBridge start/stop', () => {
  it('start registers listeners on engine', () => {
    const engine = mockEngine();
    const bridge = new EventBridge({ engine, notifier: mockNotifier() });
    bridge.start();
    assert.ok(bridge._listeners.length > 0);
  });

  it('stop removes all listeners', () => {
    const engine = mockEngine();
    const bridge = new EventBridge({ engine, notifier: mockNotifier() });
    bridge.start();
    const listenerCount = bridge._listeners.length;
    assert.ok(listenerCount > 0);
    bridge.stop();
    assert.strictEqual(bridge._listeners.length, 0);
  });

  it('start with no engine is a no-op', () => {
    const bridge = new EventBridge({ notifier: mockNotifier() });
    bridge.start(); // Should not throw
    assert.strictEqual(bridge._listeners.length, 0);
  });

  it('stop with no engine is a no-op', () => {
    const bridge = new EventBridge({ notifier: mockNotifier() });
    bridge.stop(); // Should not throw
  });
});

// ===========================================================================
// Event forwarding
// ===========================================================================

describe('EventBridge event forwarding', () => {
  it('forwards scheduler:job:completed to notifier', async () => {
    const engine = mockEngine();
    const notifier = mockNotifier();
    const bridge = new EventBridge({ engine, notifier });
    bridge.start();

    engine.emit('scheduler:job:completed', {
      job: { name: 'daily-sync' },
      result: { duration: 500 },
    });
    // Allow async handler to complete
    await new Promise((r) => setTimeout(r, 10));

    assert.strictEqual(notifier.sent.length, 1);
    assert.strictEqual(notifier.sent[0].type, 'job.completed');
    assert.ok(notifier.sent[0].message.includes('daily-sync'));
    assert.ok(notifier.sent[0].message.includes('500'));
  });

  it('forwards scheduler:job:failed', async () => {
    const engine = mockEngine();
    const notifier = mockNotifier();
    const bridge = new EventBridge({ engine, notifier });
    bridge.start();

    engine.emit('scheduler:job:failed', {
      job: { name: 'import' },
      result: { error: 'timeout' },
    });
    await new Promise((r) => setTimeout(r, 10));

    assert.ok(notifier.sent[0].message.includes('import'));
    assert.ok(notifier.sent[0].message.includes('timeout'));
  });

  it('forwards heartbeat:alert', async () => {
    const engine = mockEngine();
    const notifier = mockNotifier();
    const bridge = new EventBridge({ engine, notifier });
    bridge.start();

    engine.emit('heartbeat:alert', { checkName: 'low-stock', summary: '3 items low' });
    await new Promise((r) => setTimeout(r, 10));

    assert.strictEqual(notifier.sent[0].type, 'heartbeat.alert');
    assert.ok(notifier.sent[0].message.includes('low-stock'));
  });

  it('forwards notification event', async () => {
    const engine = mockEngine();
    const notifier = mockNotifier();
    const bridge = new EventBridge({ engine, notifier });
    bridge.start();

    engine.emit('notification', { message: 'Hello world' });
    await new Promise((r) => setTimeout(r, 10));

    assert.strictEqual(notifier.sent[0].type, 'general');
    assert.strictEqual(notifier.sent[0].message, 'Hello world');
  });
});

// ===========================================================================
// sendCommerceEvent
// ===========================================================================

describe('sendCommerceEvent', () => {
  it('sends mapped commerce event', async () => {
    const notifier = mockNotifier();
    const bridge = new EventBridge({ notifier });

    await bridge.sendCommerceEvent('order.shipped', {
      orderNumber: 'ORD-1',
      trackingNumber: 'TRK-ABC',
    });

    assert.strictEqual(notifier.sent.length, 1);
    assert.strictEqual(notifier.sent[0].type, 'order.shipped');
    assert.ok(notifier.sent[0].message.includes('ORD-1'));
    assert.ok(notifier.sent[0].message.includes('TRK-ABC'));
    assert.ok(notifier.sent[0].richMessage); // Has rich message builder
  });

  it('sends inventory.low with rich message', async () => {
    const notifier = mockNotifier();
    const bridge = new EventBridge({ notifier });

    await bridge.sendCommerceEvent('inventory.low', {
      sku: 'SKU-1',
      available: 3,
      reorderPoint: 10,
    });

    assert.strictEqual(notifier.sent[0].type, 'inventory.low');
    assert.ok(notifier.sent[0].message.includes('SKU-1'));
    assert.ok(notifier.sent[0].richMessage);
  });

  it('sends unmapped event as generic', async () => {
    const notifier = mockNotifier();
    const bridge = new EventBridge({ notifier });

    await bridge.sendCommerceEvent('custom.event', { foo: 'bar' });

    assert.strictEqual(notifier.sent[0].type, 'custom.event');
    assert.ok(notifier.sent[0].message.includes('foo'));
  });
});
