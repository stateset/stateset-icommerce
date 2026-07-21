import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  resolvePolicyStorePath,
  createEventRedactors,
  initSessionStore,
  loadSessionMeta,
  applySessionMeta,
  resolveWatchdogTimeoutMs,
  resolveAbortState,
} from '../../src/harness/run-setup.js';

describe('harness/run-setup resolvePolicyStorePath', () => {
  it('prefers an explicit override', () => {
    assert.equal(resolvePolicyStorePath('/tmp/db.sqlite', '/custom/dir'), '/custom/dir');
  });

  it('derives a .stateset dir next to the database', () => {
    assert.equal(resolvePolicyStorePath('/data/store.db'), '/data/.stateset');
  });
});

describe('harness/run-setup createEventRedactors', () => {
  it('passes values through when redactLogs is off', () => {
    const { redactEventText, redactEventValue } = createEventRedactors({ redactLogs: false });
    assert.equal(redactEventText('contact alice@example.com'), 'contact alice@example.com');
    assert.deepEqual(redactEventValue({ a: 1 }), { a: 1 });
  });

  it('redacts when redactLogs is on', () => {
    const { redactEventText } = createEventRedactors({ redactLogs: true });
    const redacted = redactEventText('contact alice@example.com now');
    assert.ok(!redacted.includes('alice@example.com'));
  });
});

describe('harness/run-setup initSessionStore', () => {
  it('returns the explicit override untouched', () => {
    const store = { get: () => null };
    assert.equal(
      initSessionStore({ sessionStore: store, resolvedSettings: {}, fallbackMaxSummaries: 5 }),
      store,
    );
  });

  it('returns null when the session store is disabled', () => {
    const instance = initSessionStore({
      sessionStore: null,
      resolvedSettings: { sessionStore: { enabled: false } },
      fallbackMaxSummaries: 5,
    });
    assert.equal(instance, null);
  });
});

describe('harness/run-setup loadSessionMeta', () => {
  const settings = { model: {} };

  it('reads metadata for a resumed session', () => {
    const meta = { model: 'stored-model' };
    const store = { get: (id) => (id === 'sess-1' ? meta : null) };
    assert.equal(
      loadSessionMeta({
        resumeSessionId: 'sess-1',
        sessionStoreInstance: store,
        resolvedSettings: settings,
      }),
      meta,
    );
  });

  it('returns null without a resume id, without a store, or when preferSession is false', () => {
    const store = {
      get: () => {
        throw new Error('should not be called');
      },
    };
    assert.equal(
      loadSessionMeta({
        resumeSessionId: null,
        sessionStoreInstance: store,
        resolvedSettings: settings,
      }),
      null,
    );
    assert.equal(
      loadSessionMeta({
        resumeSessionId: 'sess-1',
        sessionStoreInstance: null,
        resolvedSettings: settings,
      }),
      null,
    );
    assert.equal(
      loadSessionMeta({
        resumeSessionId: 'sess-1',
        sessionStoreInstance: store,
        resolvedSettings: { model: { preferSession: false } },
      }),
      null,
    );
  });

  it('degrades to null when the store read throws', () => {
    const store = {
      get: () => {
        throw new Error('db locked');
      },
    };
    assert.equal(
      loadSessionMeta({
        resumeSessionId: 'sess-1',
        sessionStoreInstance: store,
        resolvedSettings: settings,
      }),
      null,
    );
  });
});

describe('harness/run-setup applySessionMeta', () => {
  const base = {
    effectiveProvider: 'claude',
    effectiveModel: 'default-model',
    effectiveThinkLevel: 'off',
    effectiveSlaLevel: null,
  };

  it('restores stored values only for parameters the caller did not set', () => {
    const resolved = applySessionMeta({
      sessionMeta: {
        provider: 'openai',
        model: 'stored-model',
        thinkLevel: 'high',
        slaLevel: 'critical',
        agent: 'orders',
      },
      provider: undefined,
      model: undefined,
      thinkLevel: undefined,
      agent: undefined,
      ...base,
    });
    assert.equal(resolved.effectiveProvider, 'openai');
    assert.equal(resolved.effectiveModel, 'stored-model');
    assert.equal(resolved.effectiveThinkLevel, 'high');
    assert.equal(resolved.effectiveSlaLevel, 'critical');
    assert.equal(resolved.agent, 'orders');
  });

  it('never overrides explicit caller values', () => {
    const resolved = applySessionMeta({
      sessionMeta: { provider: 'openai', model: 'stored-model', agent: 'orders' },
      provider: 'claude',
      model: 'caller-model',
      thinkLevel: 'low',
      agent: 'returns',
      ...base,
      effectiveModel: 'caller-model',
      effectiveThinkLevel: 'low',
    });
    assert.equal(resolved.effectiveProvider, 'claude');
    assert.equal(resolved.effectiveModel, 'caller-model');
    assert.equal(resolved.effectiveThinkLevel, 'low');
    assert.equal(resolved.agent, 'returns');
  });

  it('is a no-op when there is no session metadata', () => {
    const resolved = applySessionMeta({
      sessionMeta: null,
      provider: undefined,
      model: undefined,
      thinkLevel: undefined,
      agent: 'inventory',
      ...base,
    });
    assert.deepEqual(resolved, { ...base, agent: 'inventory' });
  });
});

describe('harness/run-setup resolveWatchdogTimeoutMs', () => {
  const watchdogSettings = { enabled: true, freshInactivityMs: 1000, resumeInactivityMs: 2000 };

  it('uses the fresh timeout for new runs and resume timeout when resuming', () => {
    assert.equal(
      resolveWatchdogTimeoutMs({
        watchdogSettings,
        resumeSessionId: null,
        effectiveProvider: 'claude',
      }),
      1000,
    );
    assert.equal(
      resolveWatchdogTimeoutMs({
        watchdogSettings,
        resumeSessionId: 'sess-1',
        effectiveProvider: 'claude',
      }),
      2000,
    );
  });

  it('is disabled for non-claude providers, disabled settings and invalid timeouts', () => {
    assert.equal(
      resolveWatchdogTimeoutMs({
        watchdogSettings,
        resumeSessionId: null,
        effectiveProvider: 'openai',
      }),
      null,
    );
    assert.equal(
      resolveWatchdogTimeoutMs({
        watchdogSettings: { ...watchdogSettings, enabled: false },
        resumeSessionId: null,
        effectiveProvider: 'claude',
      }),
      null,
    );
    assert.equal(
      resolveWatchdogTimeoutMs({
        watchdogSettings: { enabled: true, freshInactivityMs: 0 },
        resumeSessionId: null,
        effectiveProvider: 'claude',
      }),
      null,
    );
  });
});

describe('harness/run-setup resolveAbortState', () => {
  it('uses the provided abort controller and its signal', () => {
    const controller = new AbortController();
    const { effectiveAbortController, effectiveSignal } = resolveAbortState({
      abortController: controller,
      signal: null,
      watchdogTimeoutMs: 500,
    });
    assert.equal(effectiveAbortController, controller);
    assert.equal(effectiveSignal, controller.signal);
  });

  it('creates a controller when only a watchdog needs one', () => {
    const { effectiveAbortController, effectiveSignal } = resolveAbortState({
      abortController: null,
      signal: null,
      watchdogTimeoutMs: 500,
    });
    assert.ok(effectiveAbortController instanceof AbortController);
    assert.equal(effectiveSignal, effectiveAbortController.signal);
  });

  it('returns null controller when neither controller nor watchdog is present', () => {
    const { effectiveAbortController, effectiveSignal } = resolveAbortState({
      abortController: null,
      signal: null,
      watchdogTimeoutMs: null,
    });
    assert.equal(effectiveAbortController, null);
    assert.equal(effectiveSignal, null);
  });
});
