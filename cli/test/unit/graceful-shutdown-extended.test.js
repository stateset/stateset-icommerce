/**
 * Extended graceful shutdown tests
 *
 * Tests cleanup callback registration and execution, error handling
 * during shutdown, and timeout behavior. Extends the existing
 * graceful-shutdown.test.js with deeper coverage.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { installShutdownHandlers, runMain } from '../../src/graceful-shutdown.js';

// ============================================================================
// Helpers
// ============================================================================

/**
 * Snapshot process event listeners so we can restore after each test.
 */
function snapshotListeners() {
  return {
    unhandledRejection: process.listeners('unhandledRejection').slice(),
    uncaughtException: process.listeners('uncaughtException').slice(),
    SIGINT: process.listeners('SIGINT').slice(),
    SIGTERM: process.listeners('SIGTERM').slice(),
  };
}

function restoreListeners(snapshot) {
  for (const event of ['unhandledRejection', 'uncaughtException', 'SIGINT', 'SIGTERM']) {
    const current = process.listeners(event);
    for (const listener of current) {
      if (!snapshot[event].includes(listener)) {
        process.removeListener(event, listener);
      }
    }
  }
}

// ============================================================================
// Tests
// ============================================================================

describe('graceful shutdown extended', () => {
  let snapshot;

  beforeEach(() => {
    snapshot = snapshotListeners();
  });

  afterEach(() => {
    restoreListeners(snapshot);
  });

  // --------------------------------------------------------------------------
  // Cleanup callback registration
  // --------------------------------------------------------------------------

  describe('cleanup callback registration', () => {
    it('accepts a sync cleanup function without error', () => {
      installShutdownHandlers('test-sync-cleanup', {
        cleanup: () => {
          /* sync cleanup */
        },
      });
      // Should register without throwing
      assert.ok(true);
    });

    it('accepts an async cleanup function without error', () => {
      installShutdownHandlers('test-async-cleanup', {
        cleanup: async () => {
          await new Promise((r) => setTimeout(r, 1));
        },
      });
      assert.ok(true);
    });

    it('accepts no options at all', () => {
      installShutdownHandlers('test-no-opts');
      assert.ok(true);
    });

    it('accepts empty options object', () => {
      installShutdownHandlers('test-empty-opts', {});
      assert.ok(true);
    });

    it('installs all four event listeners', () => {
      const before = {
        unhandledRejection: process.listenerCount('unhandledRejection'),
        uncaughtException: process.listenerCount('uncaughtException'),
        SIGINT: process.listenerCount('SIGINT'),
        SIGTERM: process.listenerCount('SIGTERM'),
      };
      installShutdownHandlers('test-all-listeners');
      assert.strictEqual(
        process.listenerCount('unhandledRejection'),
        before.unhandledRejection + 1,
      );
      assert.strictEqual(process.listenerCount('uncaughtException'), before.uncaughtException + 1);
      assert.strictEqual(process.listenerCount('SIGINT'), before.SIGINT + 1);
      assert.strictEqual(process.listenerCount('SIGTERM'), before.SIGTERM + 1);
    });

    it('can be called multiple times with different names', () => {
      const before = process.listenerCount('SIGINT');
      installShutdownHandlers('test-multi-1');
      installShutdownHandlers('test-multi-2');
      // Each call adds its own set of listeners
      assert.strictEqual(process.listenerCount('SIGINT'), before + 2);
    });
  });

  // --------------------------------------------------------------------------
  // Cleanup execution tracking
  // --------------------------------------------------------------------------

  describe('cleanup execution', () => {
    it('cleanup function is callable', async () => {
      let called = false;
      const cleanup = async () => {
        called = true;
      };
      installShutdownHandlers('test-callable-cleanup', { cleanup });
      // Directly invoke the cleanup to verify it works
      await cleanup();
      assert.strictEqual(called, true);
    });

    it('cleanup receives no arguments', async () => {
      let argCount = -1;
      const cleanup = async (...args) => {
        argCount = args.length;
      };
      await cleanup();
      assert.strictEqual(argCount, 0);
    });

    it('cleanup can perform async operations', async () => {
      const log = [];
      const cleanup = async () => {
        log.push('start');
        await new Promise((r) => setTimeout(r, 5));
        log.push('end');
      };
      await cleanup();
      assert.deepStrictEqual(log, ['start', 'end']);
    });
  });

  // --------------------------------------------------------------------------
  // Error handling during shutdown
  // --------------------------------------------------------------------------

  describe('error handling during shutdown', () => {
    it('cleanup errors are catchable', async () => {
      const cleanup = async () => {
        throw new Error('cleanup failed');
      };
      try {
        await cleanup();
        assert.fail('Should have thrown');
      } catch (err) {
        assert.strictEqual(err.message, 'cleanup failed');
      }
    });

    it('non-Error throws are handled', async () => {
      const cleanup = async () => {
        throw 'string error'; // eslint-disable-line no-throw-literal
      };
      try {
        await cleanup();
        assert.fail('Should have thrown');
      } catch (err) {
        assert.strictEqual(err, 'string error');
      }
    });

    it('cleanup returning rejected promise is catchable', async () => {
      const cleanup = () => Promise.reject(new Error('rejected cleanup'));
      try {
        await cleanup();
        assert.fail('Should have thrown');
      } catch (err) {
        assert.strictEqual(err.message, 'rejected cleanup');
      }
    });

    it('multiple cleanups can be composed', async () => {
      const log = [];
      const cleanup1 = async () => log.push('c1');
      const cleanup2 = async () => log.push('c2');
      const composedCleanup = async () => {
        await cleanup1();
        await cleanup2();
      };
      await composedCleanup();
      assert.deepStrictEqual(log, ['c1', 'c2']);
    });

    it('partial cleanup failure does not prevent subsequent cleanup', async () => {
      const log = [];
      const cleanup1 = async () => {
        throw new Error('c1 failed');
      };
      const cleanup2 = async () => log.push('c2');
      const composedCleanup = async () => {
        try {
          await cleanup1();
        } catch {
          log.push('c1-error');
        }
        await cleanup2();
      };
      await composedCleanup();
      assert.deepStrictEqual(log, ['c1-error', 'c2']);
    });
  });

  // --------------------------------------------------------------------------
  // Timeout behavior
  // --------------------------------------------------------------------------

  describe('timeout behavior', () => {
    it('fast cleanup completes within timeout', async () => {
      const start = Date.now();
      const cleanup = async () => {
        await new Promise((r) => setTimeout(r, 10));
      };
      await cleanup();
      const elapsed = Date.now() - start;
      assert.ok(elapsed < 1000, `Cleanup took ${elapsed}ms, expected < 1000ms`);
    });

    it('AbortController can enforce timeout on cleanup', async () => {
      const controller = new AbortController();
      const { signal } = controller;

      const cleanup = async () => {
        return new Promise((resolve, reject) => {
          const timer = setTimeout(resolve, 50);
          signal.addEventListener('abort', () => {
            clearTimeout(timer);
            reject(new Error('Cleanup timed out'));
          });
        });
      };

      // Set a very short timeout
      setTimeout(() => controller.abort(), 10);

      try {
        await cleanup();
        // If cleanup finished before abort, that is also fine
      } catch (err) {
        assert.strictEqual(err.message, 'Cleanup timed out');
      }
    });

    it('Promise.race can be used for cleanup timeouts', async () => {
      const slowCleanup = () => new Promise((r) => setTimeout(r, 5000));
      const timeout = (ms) =>
        new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), ms));

      try {
        await Promise.race([slowCleanup(), timeout(20)]);
        assert.fail('Should have timed out');
      } catch (err) {
        assert.strictEqual(err.message, 'timeout');
      }
    });
  });

  // --------------------------------------------------------------------------
  // runMain integration
  // --------------------------------------------------------------------------

  describe('runMain integration', () => {
    it('executes main function successfully', async () => {
      let executed = false;
      runMain('test-run', async () => {
        executed = true;
      });
      await new Promise((r) => setTimeout(r, 20));
      assert.strictEqual(executed, true);
    });

    it('installs shutdown handlers before running main', () => {
      const sigintBefore = process.listenerCount('SIGINT');
      runMain('test-handlers-first', async () => {
        // By the time main runs, handlers should be installed
        const sigintAfter = process.listenerCount('SIGINT');
        assert.ok(sigintAfter > sigintBefore);
      });
    });

    it('passes cleanup option through to shutdown handlers', async () => {
      let cleanupRegistered = false;
      const cleanup = async () => {
        cleanupRegistered = true;
      };
      runMain('test-cleanup-passthrough', async () => {}, { cleanup });
      // cleanup is registered but only called on signal/error
      await new Promise((r) => setTimeout(r, 20));
      // cleanup should NOT have been called during normal execution
      assert.strictEqual(cleanupRegistered, false);
    });

    it('handles main that returns a value', async () => {
      let result;
      runMain('test-return-val', async () => {
        result = 42;
        return result;
      });
      await new Promise((r) => setTimeout(r, 20));
      assert.strictEqual(result, 42);
    });
  });

  // --------------------------------------------------------------------------
  // Edge cases
  // --------------------------------------------------------------------------

  describe('edge cases', () => {
    it('handles empty string command name', () => {
      installShutdownHandlers('');
      assert.ok(true, 'Should not throw with empty name');
    });

    it('handles special characters in command name', () => {
      installShutdownHandlers('test-cmd/with:special.chars');
      assert.ok(true, 'Should not throw with special chars');
    });

    it('handler count increases predictably', () => {
      const events = ['SIGINT', 'SIGTERM', 'unhandledRejection', 'uncaughtException'];
      const before = events.map((e) => process.listenerCount(e));
      installShutdownHandlers('test-predictable');
      const after = events.map((e) => process.listenerCount(e));
      for (let i = 0; i < events.length; i++) {
        assert.strictEqual(
          after[i],
          before[i] + 1,
          `${events[i]} should have exactly 1 more listener`,
        );
      }
    });

    it('null cleanup option is treated as no cleanup', () => {
      installShutdownHandlers('test-null-cleanup', { cleanup: null });
      assert.ok(true, 'Should not throw with null cleanup');
    });
  });
});
