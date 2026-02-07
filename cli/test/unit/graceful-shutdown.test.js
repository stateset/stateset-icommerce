/**
 * Unit tests for graceful-shutdown.js
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';

// We test the module functions by importing them directly.
// Since installShutdownHandlers mutates process event listeners,
// we track and clean up to avoid contaminating other tests.

import { installShutdownHandlers, runMain } from '../../src/graceful-shutdown.js';

describe('graceful-shutdown', () => {
  let originalListeners;

  beforeEach(() => {
    // Snapshot current listeners so we can restore after each test
    originalListeners = {
      unhandledRejection: process.listeners('unhandledRejection').slice(),
      uncaughtException: process.listeners('uncaughtException').slice(),
      SIGINT: process.listeners('SIGINT').slice(),
      SIGTERM: process.listeners('SIGTERM').slice(),
    };
  });

  afterEach(() => {
    // Remove any listeners added during the test
    for (const event of ['unhandledRejection', 'uncaughtException', 'SIGINT', 'SIGTERM']) {
      const current = process.listeners(event);
      for (const listener of current) {
        if (!originalListeners[event].includes(listener)) {
          process.removeListener(event, listener);
        }
      }
    }
  });

  describe('installShutdownHandlers()', () => {
    it('registers unhandledRejection listener', () => {
      const before = process.listenerCount('unhandledRejection');
      installShutdownHandlers('test-cmd');
      assert.strictEqual(process.listenerCount('unhandledRejection'), before + 1);
    });

    it('registers uncaughtException listener', () => {
      const before = process.listenerCount('uncaughtException');
      installShutdownHandlers('test-cmd2');
      assert.strictEqual(process.listenerCount('uncaughtException'), before + 1);
    });

    it('registers SIGINT and SIGTERM listeners', () => {
      const sigintBefore = process.listenerCount('SIGINT');
      const sigtermBefore = process.listenerCount('SIGTERM');
      installShutdownHandlers('test-cmd3');
      assert.strictEqual(process.listenerCount('SIGINT'), sigintBefore + 1);
      assert.strictEqual(process.listenerCount('SIGTERM'), sigtermBefore + 1);
    });

    it('accepts optional cleanup function', () => {
      // Should not throw
      installShutdownHandlers('test-cmd4', { cleanup: async () => {} });
    });
  });

  describe('runMain()', () => {
    it('calls installShutdownHandlers and runs async main', async () => {
      let called = false;
      // We can't easily verify process.exit is called on success,
      // but we can verify the main function runs.
      const mainFn = async () => {
        called = true;
      };

      runMain('test-async', mainFn);

      // Give the microtask queue a tick to resolve
      await new Promise((r) => setTimeout(r, 10));
      assert.strictEqual(called, true);
    });

    it('calls installShutdownHandlers and runs sync main', async () => {
      let called = false;
      const mainFn = () => {
        called = true;
      };

      runMain('test-sync', mainFn);

      await new Promise((r) => setTimeout(r, 10));
      assert.strictEqual(called, true);
    });

    it('handles async main that returns a value', async () => {
      const mainFn = async () => 42;

      // Should not throw
      runMain('test-return', mainFn);
      await new Promise((r) => setTimeout(r, 10));
    });

    it('handles sync main that returns a value', async () => {
      const mainFn = () => 'hello';

      // Should not throw
      runMain('test-return-sync', mainFn);
      await new Promise((r) => setTimeout(r, 10));
    });

    it('wraps sync function with Promise.resolve', () => {
      // The key fix: runMain uses Promise.resolve(mainFn()) so sync
      // functions don't cause "cannot read property catch of undefined"
      let threw = false;
      try {
        runMain('test-promise-resolve', () => {
          /* sync noop */
        });
      } catch {
        threw = true;
      }
      assert.strictEqual(threw, false, 'runMain should not throw for sync functions');
    });

    it('passes options to installShutdownHandlers', () => {
      const sigintBefore = process.listenerCount('SIGINT');
      let cleanupCalled = false;

      runMain('test-options', async () => {}, {
        cleanup: async () => {
          cleanupCalled = true;
        },
      });

      // Verify handlers were installed
      assert.strictEqual(process.listenerCount('SIGINT'), sigintBefore + 1);
    });
  });

  describe('module exports', () => {
    it('exports installShutdownHandlers as a function', () => {
      assert.strictEqual(typeof installShutdownHandlers, 'function');
    });

    it('exports runMain as a function', () => {
      assert.strictEqual(typeof runMain, 'function');
    });
  });
});
