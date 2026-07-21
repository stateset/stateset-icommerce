import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { buildBaseHistory, prepareWorkingHistory } from '../../src/harness/context-window.js';

const noHooks = { hasHooks: () => false, run: async () => null };

describe('harness/context-window buildBaseHistory', () => {
  it('prefers explicit conversation history', () => {
    const conversationHistory = [{ role: 'user', content: 'hi' }];
    const { baseHistory, historySource } = buildBaseHistory({
      conversationHistory,
      sessionSummary: 'old summary',
    });
    assert.equal(historySource, 'conversation_history');
    assert.equal(baseHistory, conversationHistory);
  });

  it('replays a stored session summary as a seed turn', () => {
    const { baseHistory, historySource } = buildBaseHistory({
      conversationHistory: [],
      sessionSummary: 'we discussed order 42',
    });
    assert.equal(historySource, 'session_summary');
    assert.equal(baseHistory.length, 2);
    assert.equal(baseHistory[0].content, 'we discussed order 42');
    assert.equal(baseHistory[1].role, 'assistant');
  });

  it('returns empty history when nothing is available', () => {
    const { baseHistory, historySource } = buildBaseHistory({
      conversationHistory: [],
      sessionSummary: null,
    });
    assert.equal(historySource, 'none');
    assert.deepEqual(baseHistory, []);
  });
});

describe('harness/context-window prepareWorkingHistory', () => {
  it('applies transformContext and copies the base history', async () => {
    const baseHistory = [{ role: 'user', content: 'a' }];
    const { workingHistory } = await prepareWorkingHistory({
      baseHistory,
      transformContext: (history) => [...history, { role: 'assistant', content: 'b' }],
      effectiveSignal: null,
      enableContextGuard: false,
      contextSettings: {},
      effectiveModel: 'claude-test',
      effectiveRequest: 'req',
      hooks: noHooks,
    });
    assert.equal(workingHistory.length, 2);
    assert.equal(baseHistory.length, 1, 'base history must not be mutated');
  });

  it('ignores transformContext failures and non-array returns', async () => {
    const baseHistory = [{ role: 'user', content: 'a' }];
    const failing = await prepareWorkingHistory({
      baseHistory,
      transformContext: () => {
        throw new Error('boom');
      },
      effectiveSignal: null,
      enableContextGuard: false,
      contextSettings: {},
      effectiveModel: 'claude-test',
      effectiveRequest: 'req',
      hooks: noHooks,
    });
    assert.equal(failing.workingHistory.length, 1);

    const nonArray = await prepareWorkingHistory({
      baseHistory,
      transformContext: () => 'not an array',
      effectiveSignal: null,
      enableContextGuard: false,
      contextSettings: {},
      effectiveModel: 'claude-test',
      effectiveRequest: 'req',
      hooks: noHooks,
    });
    assert.equal(nonArray.workingHistory.length, 1);
  });

  it('skips the guard entirely for empty history even when enabled', async () => {
    const { contextGuardResult, compactionSummary } = await prepareWorkingHistory({
      baseHistory: [],
      transformContext: null,
      effectiveSignal: null,
      enableContextGuard: true,
      contextSettings: {},
      effectiveModel: 'claude-test',
      effectiveRequest: 'req',
      hooks: noHooks,
    });
    assert.equal(contextGuardResult, null);
    assert.equal(compactionSummary, null);
  });

  it('throws with a telemetry event when the guard aborts', async () => {
    const telemEvents = [];
    const bigHistory = Array.from({ length: 40 }, (_, i) => ({
      role: i % 2 === 0 ? 'user' : 'assistant',
      content: 'x'.repeat(5000),
    }));
    await assert.rejects(
      prepareWorkingHistory({
        baseHistory: bigHistory,
        transformContext: null,
        effectiveSignal: null,
        enableContextGuard: true,
        // Force an abort with a tiny window.
        contextSettings: {
          warningThreshold: 0.0001,
          compactThreshold: 0.0002,
          abortThreshold: 0.0003,
          reserveTokens: 0,
        },
        effectiveModel: 'claude-test',
        effectiveRequest: 'req',
        hooks: noHooks,
        telem: { logCustomEvent: (type, data) => telemEvents.push({ type, data }) },
      }),
    );
    assert.equal(telemEvents.length, 1);
    assert.equal(telemEvents[0].type, 'context_overflow');
  });

  it('short-circuits compaction when a before_compaction hook trims history below threshold', async () => {
    const bigHistory = Array.from({ length: 40 }, (_, i) => ({
      role: i % 2 === 0 ? 'user' : 'assistant',
      content: 'x'.repeat(4000),
    }));
    let afterCompactionRan = false;
    const hooks = {
      hasHooks: (name) => name === 'before_compaction' || name === 'after_compaction',
      run: async (name) => {
        if (name === 'before_compaction') {
          // Trim aggressively so the recheck no longer requires compaction.
          return { history: [{ role: 'user', content: 'tiny' }] };
        }
        if (name === 'after_compaction') afterCompactionRan = true;
        return null;
      },
    };
    const { workingHistory, compactionSummary } = await prepareWorkingHistory({
      baseHistory: bigHistory,
      transformContext: null,
      effectiveSignal: null,
      enableContextGuard: true,
      contextSettings: {
        warningThreshold: 0.0001,
        compactThreshold: 0.0002,
        abortThreshold: 0.99,
        reserveTokens: 0,
      },
      effectiveModel: 'claude-test',
      effectiveRequest: 'req',
      hooks,
    });
    assert.deepEqual(workingHistory, [{ role: 'user', content: 'tiny' }]);
    assert.equal(compactionSummary, null, 'hook short-circuit must skip compaction');
    assert.equal(afterCompactionRan, false);
  });

  it('invokes onContextWarning when the guard warns', async () => {
    const warnings = [];
    const history = Array.from({ length: 10 }, (_, i) => ({
      role: i % 2 === 0 ? 'user' : 'assistant',
      content: 'x'.repeat(4000),
    }));
    await prepareWorkingHistory({
      baseHistory: history,
      transformContext: null,
      effectiveSignal: null,
      enableContextGuard: true,
      contextSettings: {
        warningThreshold: 0.00001,
        compactThreshold: 0.95,
        abortThreshold: 0.99,
        reserveTokens: 0,
      },
      effectiveModel: 'claude-test',
      effectiveRequest: 'req',
      hooks: noHooks,
      onContextWarning: (result) => warnings.push(result),
    });
    assert.equal(warnings.length, 1);
    assert.equal(warnings[0].action, 'warn');
  });
});
