import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { CleanupTimer, CleanupListeners, CleanupManager } from '../../src/utils/cleanup.js';

describe('CleanupTimer', () => {
  let timer;

  afterEach(() => {
    if (timer) timer.clearAll();
  });

  it('tracks active setTimeout count', () => {
    timer = new CleanupTimer();
    assert.strictEqual(timer.activeCount, 0);
    timer.setTimeout(() => {}, 100000);
    assert.strictEqual(timer.activeCount, 1);
    timer.setTimeout(() => {}, 100000);
    assert.strictEqual(timer.activeCount, 2);
  });

  it('tracks active setInterval count', () => {
    timer = new CleanupTimer();
    timer.setInterval(() => {}, 100000);
    assert.strictEqual(timer.activeCount, 1);
  });

  it('clears individual timeout', () => {
    timer = new CleanupTimer();
    const id = timer.setTimeout(() => {}, 100000);
    assert.strictEqual(timer.activeCount, 1);
    timer.clearTimeout(id);
    assert.strictEqual(timer.activeCount, 0);
  });

  it('clears individual interval', () => {
    timer = new CleanupTimer();
    const id = timer.setInterval(() => {}, 100000);
    timer.clearInterval(id);
    assert.strictEqual(timer.activeCount, 0);
  });

  it('clearAll removes all timers', () => {
    timer = new CleanupTimer();
    timer.setTimeout(() => {}, 100000);
    timer.setTimeout(() => {}, 100000);
    timer.setInterval(() => {}, 100000);
    assert.strictEqual(timer.activeCount, 3);
    timer.clearAll();
    assert.strictEqual(timer.activeCount, 0);
  });

  it('auto-removes setTimeout after firing', async () => {
    timer = new CleanupTimer();
    let called = false;
    timer.setTimeout(() => {
      called = true;
    }, 10);
    assert.strictEqual(timer.activeCount, 1);
    await new Promise((r) => setTimeout(r, 50));
    assert.ok(called);
    assert.strictEqual(timer.activeCount, 0);
  });
});

describe('CleanupListeners', () => {
  it('registers and removes event listeners', () => {
    const listeners = new CleanupListeners();
    const emitter = new EventEmitter();
    let called = false;
    listeners.on(emitter, 'test', () => {
      called = true;
    });
    assert.strictEqual(listeners.activeCount, 1);

    emitter.emit('test');
    assert.ok(called);

    listeners.removeAll();
    assert.strictEqual(listeners.activeCount, 0);

    called = false;
    emitter.emit('test');
    assert.ok(!called);
  });

  it('handles once listeners', () => {
    const listeners = new CleanupListeners();
    const emitter = new EventEmitter();
    let count = 0;
    listeners.once(emitter, 'ping', () => {
      count++;
    });
    emitter.emit('ping');
    emitter.emit('ping');
    assert.strictEqual(count, 1);
  });

  it('removeAll handles already-destroyed emitters gracefully', () => {
    const listeners = new CleanupListeners();
    const emitter = new EventEmitter();
    listeners.on(emitter, 'data', () => {});
    emitter.removeAllListeners();
    // Should not throw
    listeners.removeAll();
    assert.strictEqual(listeners.activeCount, 0);
  });
});

describe('CleanupManager', () => {
  it('combines timers and listeners cleanup', async () => {
    const mgr = new CleanupManager();
    const emitter = new EventEmitter();

    mgr.setTimeout(() => {}, 100000);
    mgr.setInterval(() => {}, 100000);
    mgr.on(emitter, 'data', () => {});

    assert.strictEqual(mgr.timers.activeCount, 2);
    assert.strictEqual(mgr.listeners.activeCount, 1);
    assert.ok(!mgr.isCleanedUp);

    await mgr.cleanup();

    assert.strictEqual(mgr.timers.activeCount, 0);
    assert.strictEqual(mgr.listeners.activeCount, 0);
    assert.ok(mgr.isCleanedUp);
  });

  it('runs cleanup callbacks', async () => {
    const mgr = new CleanupManager();
    let cleaned = false;
    mgr.onCleanup(() => {
      cleaned = true;
    });
    await mgr.cleanup();
    assert.ok(cleaned);
  });

  it('runs async cleanup callbacks', async () => {
    const mgr = new CleanupManager();
    let cleaned = false;
    mgr.onCleanup(async () => {
      await new Promise((r) => setTimeout(r, 10));
      cleaned = true;
    });
    await mgr.cleanup();
    assert.ok(cleaned);
  });

  it('handles cleanup callback errors gracefully', async () => {
    const mgr = new CleanupManager();
    mgr.onCleanup(() => {
      throw new Error('cleanup failed');
    });
    mgr.onCleanup(() => {}); // second callback should still run
    // Should not throw
    await mgr.cleanup();
    assert.ok(mgr.isCleanedUp);
  });

  it('is idempotent - second cleanup is a no-op', async () => {
    const mgr = new CleanupManager();
    let count = 0;
    mgr.onCleanup(() => {
      count++;
    });
    await mgr.cleanup();
    await mgr.cleanup();
    assert.strictEqual(count, 1);
  });
});
