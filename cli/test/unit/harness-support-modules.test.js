import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  emptyUsageCounters,
  mergeUsageCounters,
  computeTotalTokens,
  normalizeCostUsd,
} from '../../src/harness/usage-counters.js';
import {
  redactStoredText,
  normalizeResponseText,
  buildErrorFields,
  writeSessionRecord,
} from '../../src/harness/session-persistence.js';
import { saveRunMemory } from '../../src/harness/memory-writer.js';
import { cloneTurnResult, summarizeToolResultsForEvent } from '../../src/harness/turn-result.js';
import { createTurnQueue } from '../../src/harness/turn-queue.js';
import { InactivityWatchdogError } from '../../src/harness-utils.js';

describe('harness/usage-counters', () => {
  it('merges snake_case and camelCase usage keys from nested sources', () => {
    const merged = mergeUsageCounters(emptyUsageCounters(), {
      usage: { input_tokens: 10, cache_read_input_tokens: 3 },
      result_usage: { outputTokens: 5, cache_creation_input_tokens: 2 },
    });
    assert.equal(merged.inputTokens, 10);
    assert.equal(merged.outputTokens, 5);
    assert.equal(merged.cacheReadTokens, 3);
    assert.equal(merged.cacheWriteTokens, 2);
    assert.equal(merged.totalTokens, 15, 'total derived from input+output');
  });

  it('preserves prior counters when a message reports nothing new', () => {
    const first = mergeUsageCounters(null, { input_tokens: 7 });
    const second = mergeUsageCounters(first, { type: 'assistant' });
    assert.equal(second.inputTokens, 7);
    assert.equal(second.outputTokens, null);
  });

  it('ignores non-numeric and empty values', () => {
    const merged = mergeUsageCounters(null, { input_tokens: 'abc', output_tokens: '' });
    assert.equal(merged.inputTokens, null);
    assert.equal(merged.outputTokens, null);
  });

  it('computeTotalTokens falls back to input+output', () => {
    assert.equal(computeTotalTokens({ totalTokens: 9, inputTokens: 1, outputTokens: 1 }), 9);
    assert.equal(computeTotalTokens({ totalTokens: null, inputTokens: 2, outputTokens: 3 }), 5);
    assert.equal(
      computeTotalTokens({ totalTokens: null, inputTokens: 2, outputTokens: null }),
      null,
    );
  });

  it('normalizeCostUsd handles empty, invalid and numeric-string costs', () => {
    assert.equal(normalizeCostUsd(null), null);
    assert.equal(normalizeCostUsd(''), null);
    assert.equal(normalizeCostUsd('not a number'), null);
    assert.equal(normalizeCostUsd('0.25'), 0.25);
    assert.equal(normalizeCostUsd(1.5), 1.5);
  });
});

describe('harness/session-persistence', () => {
  it('redactStoredText only redacts when redactMemory is enabled', () => {
    const text = 'contact alice@example.com now';
    assert.equal(redactStoredText(text, { redactMemory: false }), text);
    assert.ok(!redactStoredText(text, { redactMemory: true }).includes('alice@example.com'));
    assert.equal(redactStoredText(null, { redactMemory: true }), null);
  });

  it('normalizeResponseText coerces values and honors allowNull', () => {
    assert.equal(normalizeResponseText('hi'), 'hi');
    assert.equal(normalizeResponseText(null), '');
    assert.equal(normalizeResponseText(null, { allowNull: true }), null);
    assert.equal(normalizeResponseText(42, { allowNull: true }), '42');
  });

  it('buildErrorFields marks watchdog and abort errors as aborted runs', () => {
    const clean = buildErrorFields(null);
    assert.equal(clean.lastError, null);
    assert.equal(clean.abortedLastRun, false);

    const watchdog = buildErrorFields(new InactivityWatchdogError({ timeoutMs: 5 }));
    assert.equal(watchdog.abortedLastRun, true);
    assert.equal(watchdog.lastErrorCode, 'WATCHDOG_TIMEOUT');

    const plain = buildErrorFields(new Error('boom'));
    assert.equal(plain.lastError, 'boom');
    assert.equal(plain.abortedLastRun, false);
  });

  it('writeSessionRecord prefers recordRun and appends compaction summaries', () => {
    const calls = [];
    const store = {
      recordRun: (id, payload) => calls.push(['recordRun', id, payload]),
      upsert: (id, payload) => calls.push(['upsert', id, payload]),
      appendSummary: (id, summary) => calls.push(['appendSummary', id, summary]),
    };
    writeSessionRecord({
      sessionStoreInstance: store,
      sessionId: 's1',
      payload: { agent: 'orders' },
      compactionSummary: 'summary',
      appendCompactionSummary: true,
    });
    assert.deepEqual(
      calls.map((c) => c[0]),
      ['recordRun', 'appendSummary'],
    );
  });

  it('writeSessionRecord falls back to upsert and swallows store failures', () => {
    const calls = [];
    writeSessionRecord({
      sessionStoreInstance: { upsert: (id, payload) => calls.push([id, payload]) },
      sessionId: 's1',
      payload: { agent: 'orders' },
    });
    assert.equal(calls.length, 1);

    // Failure path must not throw.
    writeSessionRecord({
      sessionStoreInstance: {
        recordRun: () => {
          throw new Error('disk full');
        },
      },
      sessionId: 's1',
      payload: {},
    });

    // Missing store/session id are no-ops.
    writeSessionRecord({ sessionStoreInstance: null, sessionId: 's1', payload: {} });
    writeSessionRecord({
      sessionStoreInstance: { upsert: () => {} },
      sessionId: null,
      payload: {},
    });
  });
});

describe('harness/memory-writer saveRunMemory', () => {
  const baseArgs = {
    request: 'show me order 42',
    agentName: 'orders',
    sessionId: 'sess-1',
    privacySettings: { redactMemory: false },
    onError: (err) => {
      throw err;
    },
  };

  it('saves a summary entry with tool facts to both stores', async () => {
    const sqlite = [];
    const markdown = [];
    const saved = [];
    await saveRunMemory({
      ...baseArgs,
      response: 'order 42 is shipped',
      toolResults: [{ toolCall: { name: 'get_order' } }, { toolCall: null }],
      memoryStore: { save: (entry) => sqlite.push(entry) },
      markdownMemory: { save: async (entry) => markdown.push(entry) },
      onSaved: (store) => saved.push(store),
    });
    assert.equal(sqlite.length, 1);
    assert.equal(markdown.length, 1);
    assert.deepEqual(saved, ['sqlite', 'markdown']);
    assert.deepEqual(sqlite[0].facts, ['Used tool: get_order']);
    assert.ok(sqlite[0].summary.includes('show me order 42'));
  });

  it('writes an extra compaction entry when a compaction happened', async () => {
    const sqlite = [];
    await saveRunMemory({
      ...baseArgs,
      response: 'done',
      toolResults: [],
      compactionSummary: 'earlier we discussed X',
      memoryStore: { save: (entry) => sqlite.push(entry) },
      markdownMemory: null,
    });
    assert.equal(sqlite.length, 2);
    assert.ok(sqlite[0].facts.includes('Context compaction applied'));
    assert.ok(sqlite[1].summary.startsWith('[Compaction]'));
  });

  it('skips entirely without a response and reports store failures via onError', async () => {
    let saved = false;
    await saveRunMemory({
      ...baseArgs,
      response: '',
      memoryStore: {
        save: () => {
          saved = true;
        },
      },
      markdownMemory: null,
    });
    assert.equal(saved, false);

    const errors = [];
    await saveRunMemory({
      ...baseArgs,
      onError: (err) => errors.push(err),
      response: 'ok',
      memoryStore: {
        save: () => {
          throw new Error('sqlite locked');
        },
      },
      markdownMemory: null,
    });
    assert.equal(errors.length, 1);
    assert.equal(errors[0].message, 'sqlite locked');
  });
});

describe('harness/turn-result', () => {
  it('cloneTurnResult deep-copies nested structures', () => {
    const original = {
      response: 'hi',
      usage: { inputTokens: 1 },
      promptReport: { tokens: 2 },
      sessionRefresh: { refreshed: true },
      treasury: { requestId: 'r1', charge: { amount: '1' }, identity: { agent_id: 'a' } },
      toolResults: [{ toolCall: { id: 't1' }, result: { ok: true } }],
    };
    const clone = cloneTurnResult(original);
    clone.usage.inputTokens = 99;
    clone.treasury.charge.amount = '9';
    clone.toolResults[0].toolCall.id = 'mutated';
    assert.equal(original.usage.inputTokens, 1);
    assert.equal(original.treasury.charge.amount, '1');
    assert.equal(original.toolResults[0].toolCall.id, 't1');
    assert.equal(cloneTurnResult(null), null);
  });

  it('summarizeToolResultsForEvent keeps only id/name/input plus result and duration', () => {
    const summary = summarizeToolResultsForEvent([
      {
        toolCall: { id: 't1', name: 'get_order', input: { id: 1 }, startTime: 123 },
        result: { ok: true },
        duration: 5,
      },
      { toolCall: null, result: null },
    ]);
    assert.deepEqual(summary[0], {
      toolCall: { id: 't1', name: 'get_order', input: { id: 1 } },
      result: { ok: true },
      duration: 5,
    });
    assert.deepEqual(summary[1], { toolCall: null, result: null, duration: null });
  });
});

describe('harness/turn-queue', () => {
  it('prioritizes steer over followUp over plain sends', async () => {
    const q = createTurnQueue();
    q.enqueue('plain', 'send');
    q.enqueue('follow', 'followUp');
    q.enqueue('steer', 'steer');
    assert.equal(await q.nextMessage(), 'steer');
    assert.equal(await q.nextMessage(), 'follow');
    assert.equal(await q.nextMessage(), 'plain');
  });

  it('holds messages while a turn is in flight and wakes on notify', async () => {
    const q = createTurnQueue();
    q.setInTurn(true);
    q.enqueue('queued');
    let resolved = null;
    const pending = q.nextMessage().then((value) => {
      resolved = value;
    });
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.equal(resolved, null, 'must not dequeue mid-turn');
    q.setInTurn(false);
    q.notify();
    await pending;
    assert.equal(resolved, 'queued');
  });

  it('ignores empty messages and resolves null after close', async () => {
    const q = createTurnQueue();
    q.enqueue('');
    const pending = q.nextMessage();
    q.close();
    assert.equal(await pending, null);
    assert.equal(q.isClosed(), true);
    assert.equal(await q.nextMessage(), null);
  });
});
