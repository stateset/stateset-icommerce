/**
 * Turn-result shaping helpers for createAgentStreamSession.
 *
 * Pure functions extracted from claude-harness.js — no module-scope state.
 */

/** Deep-ish clone of a turn result so callers can't mutate internal state. */
export const cloneTurnResult = (result) => {
  if (!result) return null;
  return {
    ...result,
    usage: result.usage ? { ...result.usage } : null,
    promptReport: result.promptReport ? { ...result.promptReport } : null,
    sessionRefresh: result.sessionRefresh ? { ...result.sessionRefresh } : null,
    treasury: result.treasury
      ? {
          ...result.treasury,
          charge: result.treasury.charge ? { ...result.treasury.charge } : null,
          identity: result.treasury.identity ? { ...result.treasury.identity } : null,
        }
      : result.treasury,
    toolResults: Array.isArray(result.toolResults)
      ? result.toolResults.map((entry) => ({
          ...entry,
          toolCall: entry.toolCall ? { ...entry.toolCall } : entry.toolCall,
        }))
      : [],
  };
};

/** Compact tool-result summary attached to turn_end/agent_end events. */
export const summarizeToolResultsForEvent = (toolResults = []) =>
  toolResults.map((entry) => ({
    toolCall: entry.toolCall
      ? {
          id: entry.toolCall.id,
          name: entry.toolCall.name,
          input: entry.toolCall.input,
        }
      : null,
    result: entry.result,
    duration: entry.duration ?? null,
  }));
