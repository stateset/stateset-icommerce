/**
 * Unit tests for harness-utils.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { buildClaudeEnv, emitEvent, normalizeAbortController } from '../../src/harness-utils.js';

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

describe('emitEvent', () => {
  it('is a no-op when onEvent is not a function', () => {
    assert.doesNotThrow(() => emitEvent(null, { type: 'x' }));
  });

  it('invokes callback with event payload', () => {
    let received = null;
    emitEvent((event) => {
      received = event;
    }, { type: 'event', value: 42 });
    assert.deepStrictEqual(received, { type: 'event', value: 42 });
  });

  it('catches synchronous callback errors', () => {
    const originalError = console.error;
    const calls = [];
    console.error = (...args) => calls.push(args);
    try {
      assert.doesNotThrow(() => {
        emitEvent(() => {
          throw new Error('boom');
        }, { type: 'sync-fail' });
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
      emitEvent(async () => {
        throw new Error('async-boom');
      }, { type: 'async-fail' });
      await new Promise((resolve) => setImmediate(resolve));
      assert.ok(calls.length >= 1);
    } finally {
      console.error = originalError;
    }
  });
});
