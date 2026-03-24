export function formatSessionRefreshReason(reason = 'session_refresh') {
  return String(reason || 'session_refresh').replace(/_/g, ' ');
}

export function formatSessionRefreshTimestamp(recordedAt) {
  if (!recordedAt) return 'n/a';
  const date = new Date(recordedAt);
  if (Number.isNaN(date.getTime())) return String(recordedAt);
  return date
    .toISOString()
    .replace(/\.\d{3}Z$/, 'Z')
    .replace('T', ' ');
}

export function appendSessionRefresh(history = [], refresh, { maxEntries = 10 } = {}) {
  const nextHistory = Array.isArray(history) ? history.map((entry) => ({ ...entry })) : [];
  if (!refresh) {
    return nextHistory;
  }

  const previousSequence = Number(nextHistory[nextHistory.length - 1]?.sequence) || 0;
  const replayedMessages = Number.parseInt(refresh.replayedMessages, 10);
  nextHistory.push({
    sequence: previousSequence + 1,
    reason: refresh.reason || 'session_refresh',
    previousSessionId: refresh.previousSessionId || null,
    sessionId: refresh.sessionId || null,
    replayedMessages:
      Number.isFinite(replayedMessages) && replayedMessages > 0 ? replayedMessages : 0,
    recordedAt: refresh.recordedAt || new Date().toISOString(),
  });

  if (Number.isFinite(maxEntries) && maxEntries > 0 && nextHistory.length > maxEntries) {
    return nextHistory.slice(-maxEntries);
  }

  return nextHistory;
}
