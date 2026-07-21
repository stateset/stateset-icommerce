/**
 * Dual memory-store persistence (SQLite + Markdown) for the Claude harness.
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected;
 * there is no module-scope state.
 */

import { redactSensitive } from '../privacy.js';

/**
 * Save a completed run to the memory stores. Extracts key facts from tool
 * usage, applies memory redaction and (when a compaction happened) writes an
 * additional compaction entry. Errors are reported via onError and swallowed.
 *
 * @param {Object} deps
 * @param {Function|null} deps.onSaved - Called with the store kind ('sqlite'|'markdown') after each save.
 * @param {Function} deps.onError - Called with the error on failure.
 */
export async function saveRunMemory({
  request,
  response,
  toolResults = [],
  compactionSummary = null,
  agentName,
  sessionId,
  memoryStore,
  markdownMemory,
  privacySettings,
  onSaved = null,
  onError,
}) {
  if (!response) return;
  try {
    // Extract key facts from the conversation
    const facts = [];
    for (const tr of toolResults) {
      if (tr?.toolCall?.name) {
        facts.push(`Used tool: ${tr.toolCall.name}`);
      }
    }

    if (compactionSummary) {
      facts.push('Context compaction applied');
    }

    const summaryRequest = privacySettings.redactMemory
      ? redactSensitive(request, privacySettings)
      : request;
    const summaryResponse = privacySettings.redactMemory
      ? redactSensitive(response, privacySettings)
      : response;

    const memoryEntry = {
      summary: `${summaryRequest.slice(0, 100)}${summaryRequest.length > 100 ? '...' : ''} → ${summaryResponse.slice(0, 150)}${summaryResponse.length > 150 ? '...' : ''}`,
      facts,
      agent: agentName,
      sessionId,
      channel: 'cli',
      senderId: 'local',
    };

    // Save to SQLite memory store
    if (memoryStore) {
      memoryStore.save(memoryEntry);
      if (onSaved) onSaved('sqlite');
    }

    // Save to markdown memory store
    if (markdownMemory) {
      await markdownMemory.save(memoryEntry);
      if (onSaved) onSaved('markdown');
    }

    if (compactionSummary) {
      const compactionEntry = {
        summary: `[Compaction] ${summaryRequest.slice(0, 80)}...`,
        facts: [`Summary: ${compactionSummary.slice(0, 200)}`],
        agent: agentName,
        sessionId,
        channel: 'cli',
        senderId: 'local',
      };
      if (memoryStore) memoryStore.save(compactionEntry);
      if (markdownMemory) await markdownMemory.save(compactionEntry);
    }
  } catch (e) {
    onError(e);
  }
}
