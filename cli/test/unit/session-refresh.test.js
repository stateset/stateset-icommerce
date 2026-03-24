import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  appendSessionRefresh,
  formatSessionRefreshReason,
  formatSessionRefreshTimestamp,
} from '../../src/utils/session-refresh.js';

describe('session refresh helpers', () => {
  it('formats session refresh reasons for operator output', () => {
    assert.equal(formatSessionRefreshReason('treasury_budget_refresh'), 'treasury budget refresh');
    assert.equal(formatSessionRefreshReason(), 'session refresh');
  });

  it('formats ISO timestamps without milliseconds', () => {
    assert.equal(formatSessionRefreshTimestamp('2026-03-23T10:20:30.123Z'), '2026-03-23 10:20:30Z');
    assert.equal(formatSessionRefreshTimestamp(null), 'n/a');
    assert.equal(formatSessionRefreshTimestamp('not-a-date'), 'not-a-date');
  });

  it('appends normalized refresh records and preserves bounded history ordering', () => {
    const first = appendSessionRefresh([], {
      reason: 'treasury_budget_refresh',
      previousSessionId: 'sess-1',
      sessionId: 'sess-2',
      replayedMessages: '4',
      recordedAt: '2026-03-23T10:20:30.123Z',
    });

    assert.deepEqual(first, [
      {
        sequence: 1,
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-1',
        sessionId: 'sess-2',
        replayedMessages: 4,
        recordedAt: '2026-03-23T10:20:30.123Z',
      },
    ]);

    const second = appendSessionRefresh(first, {
      reason: 'session_reset',
      previousSessionId: 'sess-2',
      sessionId: 'sess-3',
      replayedMessages: -2,
      recordedAt: '2026-03-23T10:21:00.000Z',
    });
    const third = appendSessionRefresh(
      second,
      {
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-3',
        sessionId: 'sess-4',
        replayedMessages: 2,
        recordedAt: '2026-03-23T10:22:00.000Z',
      },
      { maxEntries: 2 },
    );

    assert.deepEqual(third, [
      {
        sequence: 2,
        reason: 'session_reset',
        previousSessionId: 'sess-2',
        sessionId: 'sess-3',
        replayedMessages: 0,
        recordedAt: '2026-03-23T10:21:00.000Z',
      },
      {
        sequence: 3,
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-3',
        sessionId: 'sess-4',
        replayedMessages: 2,
        recordedAt: '2026-03-23T10:22:00.000Z',
      },
    ]);
  });
});
