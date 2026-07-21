import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { executeWithRetry } from '../../src/harness/retry-loop.js';
import { InactivityWatchdogError } from '../../src/harness-utils.js';

const telemStub = () => {
  const events = [];
  return {
    events,
    logCustomEvent(type, data) {
      events.push({ type, data });
    },
  };
};

const okResult = (extra = {}) => ({
  toolResults: [],
  response: 'ok',
  error: null,
  errorType: null,
  ...extra,
});

describe('harness/retry-loop executeWithRetry', () => {
  it('returns the result on first success without telemetry', async () => {
    const telem = telemStub();
    const result = await executeWithRetry({
      executeOnce: async () => okResult(),
      retrySettings: { enabled: true, maxRetries: 3 },
      telem,
    });
    assert.equal(result.response, 'ok');
    assert.equal(telem.events.length, 0);
  });

  it('exits without retrying when budget is exceeded (error_max_budget_usd)', async () => {
    const telem = telemStub();
    let calls = 0;
    const result = await executeWithRetry({
      executeOnce: async () => {
        calls++;
        return okResult({
          error: 'error_max_budget_usd',
          errorType: 'error_max_budget_usd',
          budgetExceeded: true,
        });
      },
      retrySettings: { enabled: true, maxRetries: 3 },
      telem,
    });
    assert.equal(calls, 1);
    assert.equal(result.errorType, 'error_max_budget_usd');
    assert.equal(result.budgetExceeded, true);
    assert.equal(telem.events.length, 0);
  });

  it('throws a coded error for non-budget error results without retrying non-retryables', async () => {
    const telem = telemStub();
    await assert.rejects(
      executeWithRetry({
        executeOnce: async () =>
          okResult({ error: 'max turns reached', errorType: 'error_max_turns' }),
        retrySettings: { enabled: true, maxRetries: 3, retryableErrors: ['rate limit'] },
        telem,
      }),
      (err) => {
        assert.equal(err.message, 'max turns reached');
        assert.equal(err.code, 'error_max_turns');
        return true;
      },
    );
    assert.equal(telem.events.length, 0);
  });

  it('never retries watchdog timeouts even when retries are enabled', async () => {
    const telem = telemStub();
    let calls = 0;
    await assert.rejects(
      executeWithRetry({
        executeOnce: async () => {
          calls++;
          throw new InactivityWatchdogError({ timeoutMs: 5, elapsedMs: 6 });
        },
        retrySettings: { enabled: true, maxRetries: 5 },
        telem,
      }),
      InactivityWatchdogError,
    );
    assert.equal(calls, 1);
  });

  it('retries retryable errors with telemetry and eventually succeeds', async () => {
    const telem = telemStub();
    let calls = 0;
    const result = await executeWithRetry({
      executeOnce: async () => {
        calls++;
        if (calls < 3) {
          const err = new Error('rate limit exceeded (429)');
          err.status = 429;
          throw err;
        }
        return okResult({ response: 'recovered' });
      },
      retrySettings: {
        enabled: true,
        maxRetries: 3,
        baseDelayMs: 1,
        maxDelayMs: 2,
        retryableErrors: ['rate limit'],
      },
      telem,
    });
    assert.equal(calls, 3);
    assert.equal(result.response, 'recovered');
    const retryEvents = telem.events.filter((e) => e.type === 'auto_retry');
    assert.equal(retryEvents.length, 2);
    assert.equal(retryEvents[0].data.attempt, 1);
  });

  it('rethrows when retries are exhausted', async () => {
    const telem = telemStub();
    let calls = 0;
    await assert.rejects(
      executeWithRetry({
        executeOnce: async () => {
          calls++;
          const err = new Error('rate limit exceeded (429)');
          err.status = 429;
          throw err;
        },
        retrySettings: {
          enabled: true,
          maxRetries: 1,
          baseDelayMs: 1,
          maxDelayMs: 1,
          retryableErrors: ['rate limit'],
        },
        telem,
      }),
      /rate limit/,
    );
    assert.equal(calls, 2);
  });

  it('does not retry when retries are disabled', async () => {
    const telem = telemStub();
    let calls = 0;
    await assert.rejects(
      executeWithRetry({
        executeOnce: async () => {
          calls++;
          const err = new Error('rate limit exceeded (429)');
          err.status = 429;
          throw err;
        },
        retrySettings: { enabled: false, maxRetries: 3, retryableErrors: ['rate limit'] },
        telem,
      }),
      /rate limit/,
    );
    assert.equal(calls, 1);
  });
});
