/**
 * Unit tests for harness-utils.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  buildClaudeEnv,
  createInactivityWatchdog,
  emitEvent,
  InactivityWatchdogError,
  isAbortLikeError,
  normalizeAbortController,
} from '../../src/harness-utils.js';

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

describe('buildClaudeEnv', () => {
  it('applies explicit apiKey override', () => {
    const env = buildClaudeEnv({
      env: { FOO: 'bar' },
      apiKey: 'explicit-key',
    });
    assert.strictEqual(env.FOO, 'bar');
    assert.strictEqual(env.ANTHROPIC_API_KEY, 'explicit-key');
  });

  it('preserves existing ANTHROPIC_API_KEY in provided env', () => {
    const env = buildClaudeEnv({
      env: { ANTHROPIC_API_KEY: 'existing-key' },
    });
    assert.strictEqual(env.ANTHROPIC_API_KEY, 'existing-key');
  });
});

describe('normalizeAbortController', () => {
  it('returns provided abortController as-is', () => {
    const controller = new AbortController();
    const result = normalizeAbortController({ abortController: controller });
    assert.strictEqual(result, controller);
  });

  it('returns null when neither signal nor controller are provided', () => {
    const result = normalizeAbortController({});
    assert.strictEqual(result, null);
  });

  it('returns aborted controller when signal is already aborted', () => {
    const parent = new AbortController();
    parent.abort('done');
    const child = normalizeAbortController({ signal: parent.signal });
    assert.ok(child);
    assert.strictEqual(child.signal.aborted, true);
  });

  it('forwards future abort from signal to returned controller', async () => {
    const parent = new AbortController();
    const child = normalizeAbortController({ signal: parent.signal });
    assert.ok(child);
    assert.strictEqual(child.signal.aborted, false);

    parent.abort('stop');
    await new Promise((resolve) => setImmediate(resolve));
    assert.strictEqual(child.signal.aborted, true);
  });
});

describe('createInactivityWatchdog', () => {
  it('aborts the controller after inactivity', async () => {
    const controller = new AbortController();
    let timeoutError = null;
    const watchdog = createInactivityWatchdog({
      timeoutMs: 25,
      abortController: controller,
      onTimeout: (error) => {
        timeoutError = error;
      },
    });

    await sleep(60);

    assert.ok(timeoutError instanceof InactivityWatchdogError);
    assert.strictEqual(timeoutError.code, 'WATCHDOG_TIMEOUT');
    assert.strictEqual(controller.signal.aborted, true);
    assert.strictEqual(controller.signal.reason, timeoutError);
    assert.strictEqual(watchdog.timedOut, true);
    watchdog.stop();
  });

  it('resets the inactivity window when touched', async () => {
    const watchdog = createInactivityWatchdog({ timeoutMs: 35 });

    await sleep(20);
    watchdog.touch();
    await sleep(25);
    assert.strictEqual(watchdog.timedOut, false);

    await sleep(25);
    assert.strictEqual(watchdog.timedOut, true);
    watchdog.stop();
  });

  it('can be stopped before timeout', async () => {
    const watchdog = createInactivityWatchdog({ timeoutMs: 20 });
    watchdog.stop();
    await sleep(40);
    assert.strictEqual(watchdog.timedOut, false);
    assert.strictEqual(watchdog.error, null);
  });
});

describe('isAbortLikeError', () => {
  it('recognizes abort errors directly and through causes', () => {
    const abortError = new Error('The operation was aborted');
    abortError.name = 'AbortError';
    assert.strictEqual(isAbortLikeError(abortError), true);

    const wrappedError = new Error('outer');
    wrappedError.cause = abortError;
    assert.strictEqual(isAbortLikeError(wrappedError), true);
    assert.strictEqual(isAbortLikeError(new Error('timeout exceeded')), false);
  });
});

describe('emitEvent', () => {
  it('is a no-op when onEvent is not a function', () => {
    assert.doesNotThrow(() => emitEvent(null, { type: 'x' }));
  });

  it('invokes callback with event payload', () => {
    let received = null;
    emitEvent(
      (event) => {
        received = event;
      },
      { type: 'event', value: 42 },
    );
    assert.deepStrictEqual(received, { type: 'event', value: 42 });
  });

  it('catches synchronous callback errors', () => {
    const originalError = console.error;
    const calls = [];
    console.error = (...args) => calls.push(args);
    try {
      assert.doesNotThrow(() => {
        emitEvent(
          () => {
            throw new Error('boom');
          },
          { type: 'sync-fail' },
        );
      });
      assert.ok(calls.length >= 1);
    } finally {
      console.error = originalError;
    }
  });

  it('catches asynchronous callback rejections', async () => {
    const originalError = console.error;
    const calls = [];
    console.error = (...args) => calls.push(args);
    try {
      emitEvent(
        async () => {
          throw new Error('async-boom');
        },
        { type: 'async-fail' },
      );
      await new Promise((resolve) => setImmediate(resolve));
      assert.ok(calls.length >= 1);
    } finally {
      console.error = originalError;
    }
  });
});
