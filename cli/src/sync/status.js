/**
 * Shared operator status for the engine, direct commands and MCP tools.
 * Receive cursors include quarantined/retained records, not just verified ones.
 */
export function readSyncStatus(outbox, { connected = false, remoteHead } = {}) {
  outbox.initialize();
  return outbox.db.transaction(() => {
    const state = outbox.getSyncState();
    const stats = outbox.getStats();
    const receive = outbox.getReceiveSummary();
    const reasons = [];
    if (!connected) reasons.push('offline');

    // lastPulledSequence stores the next inclusive request cursor.
    const nextPullCursor = state.lastPulledSequence;
    const localHead = Math.max(0, nextPullCursor - 1);
    const reportedHead = connected ? remoteHead : state.headSequence;
    const validHead = Number.isSafeInteger(reportedHead) && reportedHead >= 0;
    if (!validHead) reasons.push('invalid_remote_head');
    if (validHead && connected && reportedHead < state.headSequence) {
      reasons.push('remote_head_regressed');
    }
    const effectiveHead = validHead ? reportedHead : state.headSequence;
    // This is a sequence distance, not an event count: scoped feeds have gaps.
    const lag = Math.max(0, effectiveHead - localHead);
    if (lag >= 100) reasons.push('pull_lag');
    if (stats.pending >= 1000) reasons.push('pending_backlog');
    if (stats.failed > 0) reasons.push('failed_outgoing_events');
    if (stats.rejected > 0) reasons.push('rejected_outgoing_events');
    if (receive.quarantined > 0) reasons.push('quarantined_events');
    if (receive.failures.count > 0) reasons.push('retained_receive_failures');

    return {
      connected,
      localState: {
        lastPushedSequence: state.lastPushedSequence,
        lastPulledSequence: state.lastPulledSequence,
        lastSyncAt: state.lastSyncAt,
        lastPullAt: state.lastPullAt,
      },
      localHead,
      nextPullCursor,
      remoteHead: effectiveHead,
      lag,
      pending: stats.pending ?? 0,
      outbox: stats,
      receive,
      health: !connected ? 'offline' : reasons.length ? 'degraded' : 'healthy',
      healthReasons: reasons,
    };
  })();
}
