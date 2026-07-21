/**
 * Conversation-history assembly and context-window guard handling for the
 * Claude harness (shared by runAgentLoop and runAgentStream).
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected;
 * there is no module-scope state.
 */

import { ContextGuard } from '../context-guard.js';
import { extractCompactionSummary } from '../conversation-history.js';

/**
 * Build the base history for a run: explicit conversation history wins,
 * otherwise the latest stored session summary is replayed as a seed turn.
 */
export function buildBaseHistory({ conversationHistory, sessionSummary }) {
  const historySource =
    conversationHistory.length > 0
      ? 'conversation_history'
      : sessionSummary
        ? 'session_summary'
        : 'none';
  const baseHistory =
    conversationHistory.length > 0
      ? conversationHistory
      : sessionSummary
        ? [
            { role: 'user', content: sessionSummary },
            {
              role: 'assistant',
              content: 'Understood. I have the context from our earlier conversation.',
            },
          ]
        : [];
  return { baseHistory, historySource };
}

/**
 * Apply the optional transformContext callback and, when enabled, the
 * context-window guard (warn / compact / abort) including the
 * before_compaction and after_compaction hooks.
 *
 * @returns {Promise<{workingHistory: object[], contextGuardResult: object|null, compactionSummary: string|null}>}
 */
export async function prepareWorkingHistory({
  baseHistory,
  transformContext,
  effectiveSignal,
  enableContextGuard,
  contextSettings,
  effectiveModel,
  effectiveRequest,
  hooks,
  telem = null,
  onContextWarning = null,
}) {
  let workingHistory = [...baseHistory];
  let contextGuardResult = null;
  let compactionSummary = null;

  if (typeof transformContext === 'function') {
    try {
      const transformed = await transformContext(workingHistory, effectiveSignal);
      if (Array.isArray(transformed)) {
        workingHistory = transformed;
      }
    } catch (err) {
      console.warn('[Harness] transformContext failed:', err.message);
    }
  }

  const abortOnOverflow = (result) => {
    if (telem) {
      telem.logCustomEvent('context_overflow', {
        tokens: result.usage.tokens,
        percent: result.usage.percent,
      });
    }
    throw new Error(result.message);
  };

  if (enableContextGuard && workingHistory.length > 0) {
    const contextGuard = ContextGuard.forModel(effectiveModel, {
      warningThreshold: contextSettings.warningThreshold,
      compactThreshold: contextSettings.compactThreshold,
      abortThreshold: contextSettings.abortThreshold,
      reserveTokens: contextSettings.reserveTokens,
    });
    contextGuardResult = contextGuard.check(
      workingHistory,
      '', // System prompt will be added by SDK
      effectiveRequest,
    );

    if (!contextGuardResult.safe && contextGuardResult.action === 'abort') {
      abortOnOverflow(contextGuardResult);
    }

    if (contextGuardResult.action === 'compact') {
      let historyForCompaction = workingHistory;
      if (hooks?.hasHooks?.('before_compaction')) {
        const hookResult = await hooks.run('before_compaction', {
          history: historyForCompaction,
          usage: contextGuardResult.usage,
          request: effectiveRequest,
        });
        if (hookResult?.history) {
          historyForCompaction = hookResult.history;
        }
      }

      if (historyForCompaction !== workingHistory) {
        workingHistory = historyForCompaction;
        contextGuardResult = contextGuard.check(workingHistory, '', effectiveRequest);
        if (!contextGuardResult.safe && contextGuardResult.action === 'abort') {
          abortOnOverflow(contextGuardResult);
        }
      }

      if (contextGuardResult.action !== 'compact') {
        // Recheck after hook no longer requires compaction.
      } else {
        workingHistory = contextGuardResult.compactedHistory;
        compactionSummary = extractCompactionSummary(contextGuardResult.compactedHistory);
        if (telem) {
          telem.logCustomEvent('context_compacted', {
            originalTokens: contextGuardResult.usage.tokens,
            compactedTokens: contextGuardResult.usage.afterCompaction?.tokens,
            tokensSaved: contextGuardResult.usage.afterCompaction?.tokensSaved,
          });
        }
        if (hooks?.hasHooks?.('after_compaction')) {
          await hooks.run('after_compaction', {
            summary: compactionSummary,
            usage: contextGuardResult.usage,
          });
        }
      }
    }

    if (contextGuardResult.action === 'warn' && onContextWarning) {
      onContextWarning(contextGuardResult);
    }
  }

  return { workingHistory, contextGuardResult, compactionSummary };
}
