/**
 * Claude SDK query execution for runAgentLoop: consumes the SDK message
 * stream and assembles tool calls, assistant text, usage counters, budget
 * and error state for a single attempt.
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected
 * (including the SDK invocation itself via `executeQuery`); there is no
 * module-scope state. The `syncState` callbacks mirror assignments the
 * original closure made to runAgentLoop locals so that error paths still
 * persist partial progress.
 */

import { emitEvent, createInactivityWatchdog, isAbortLikeError } from '../harness-utils.js';
import { redactObject } from '../privacy.js';
import { emptyUsageCounters, mergeUsageCounters } from './usage-counters.js';

export function createRunQuery({
  executeQuery,
  options,
  requestWithHistory,
  resumeSessionId,
  watchdogTimeoutMs,
  effectiveAbortController,
  effectiveProvider,
  telem,
  onEvent,
  redactEventText,
  redactEventValue,
  hooks,
  privacySettings,
  streaming,
  onToolCall,
  onPartialMessage,
  onThinkingBlock,
  syncState,
}) {
  return async function runQuery(queryModel) {
    const queryOptions = { ...options, model: queryModel };
    const results = {
      toolResults: [],
      response: '',
      sessionId: null,
      budgetExceeded: false,
      totalCost: null,
      error: null,
      errorType: null,
      usage: emptyUsageCounters(),
    };
    const pendingToolCalls = new Map();
    let assistantStarted = false;
    let assistantText = '';
    syncState.onAttemptStart();
    const watchdog = watchdogTimeoutMs
      ? createInactivityWatchdog({
          timeoutMs: watchdogTimeoutMs,
          abortController: effectiveAbortController,
          message: resumeSessionId
            ? `No Claude SDK activity while resuming session after ${watchdogTimeoutMs}ms`
            : `No Claude SDK activity received after ${watchdogTimeoutMs}ms`,
          onTimeout: (watchdogError) => {
            const currentSessionId =
              results.sessionId || syncState.getSessionId() || resumeSessionId || null;
            telem.logCustomEvent('watchdog_timeout', {
              timeoutMs: watchdogTimeoutMs,
              elapsedMs: watchdogError.elapsedMs,
              provider: effectiveProvider,
              model: queryModel,
              sessionId: currentSessionId,
            });
            emitEvent(onEvent, {
              type: 'watchdog_timeout',
              timeoutMs: watchdogTimeoutMs,
              elapsedMs: watchdogError.elapsedMs,
              provider: effectiveProvider,
              model: queryModel,
              sessionId: currentSessionId,
            });
          },
        })
      : null;

    try {
      for await (const message of executeQuery({
        prompt: requestWithHistory,
        options: queryOptions,
      })) {
        watchdog?.touch();
        results.usage = mergeUsageCounters(results.usage, message);
        syncState.setUsage(results.usage);

        if (message.sessionId && !results.sessionId) {
          results.sessionId = message.sessionId;
        }
        if (message.sessionId) {
          syncState.setSessionId(message.sessionId);
        }

        if (message.type === 'assistant') {
          const content = message.message?.content || message.content;
          if (content) {
            if (!assistantStarted) {
              assistantStarted = true;
              emitEvent(onEvent, {
                type: 'message_start',
                message: { role: 'assistant', content: '' },
              });
            }
            for (const block of content) {
              if (block.type === 'tool_use') {
                const toolCall = {
                  id: block.id,
                  name: block.name,
                  input: block.input,
                  startTime: Date.now(),
                };
                const entry = { toolCall, result: null };
                results.toolResults.push(entry);
                if (toolCall.id) {
                  pendingToolCalls.set(toolCall.id, entry);
                }
                emitEvent(onEvent, {
                  type: 'tool_execution_start',
                  toolCallId: toolCall.id,
                  toolName: toolCall.name,
                  args: redactEventValue(toolCall.input),
                });
                if (onToolCall) {
                  onToolCall(toolCall);
                }
              } else if (block.type === 'text') {
                results.response += block.text;
                assistantText = results.response;
                syncState.setResponse(results.response);
                if (streaming) {
                  emitEvent(onEvent, {
                    type: 'message_update',
                    message: { role: 'assistant', content: redactEventText(assistantText) },
                    delta: redactEventText(block.text),
                  });
                }
              } else if (block.type === 'thinking' && onThinkingBlock) {
                onThinkingBlock(block);
              }
            }
          }
        } else if (message.type === 'result') {
          results.usage = mergeUsageCounters(results.usage, message);
          syncState.setUsage(results.usage);
          if (message.result) {
            results.response = message.result;
            assistantText = message.result;
            syncState.setResponse(message.result);
          }
          if (message.total_cost_usd !== null && message.total_cost_usd !== undefined) {
            results.totalCost = message.total_cost_usd;
          }
          if (message.subtype === 'error_max_budget_usd') {
            results.budgetExceeded = true;
          }
          if (message.subtype && message.subtype.startsWith('error_')) {
            results.errorType = message.subtype;
            results.error =
              message.errors && message.errors.length > 0
                ? message.errors.join('; ')
                : message.subtype;
          }
          if (!assistantStarted) {
            assistantStarted = true;
            emitEvent(onEvent, {
              type: 'message_start',
              message: { role: 'assistant', content: '' },
            });
          }
          if (assistantText) {
            emitEvent(onEvent, {
              type: 'message_end',
              message: { role: 'assistant', content: redactEventText(assistantText) },
            });
          }
        } else if (message.type === 'user') {
          const toolUseId =
            message.parent_tool_use_id ||
            message.tool_use_id ||
            message.tool_use_result?.tool_use_id ||
            message.tool_use_result?.tool_use_id;
          const pending = toolUseId
            ? pendingToolCalls.get(toolUseId)
            : results.toolResults.find((tr) => tr.result === null);
          if (pending && message.tool_use_result) {
            pending.result = message.tool_use_result;
            pending.endTime = Date.now();
            pending.duration = pending.endTime - pending.toolCall.startTime;
            if (hooks?.hasHooks?.('tool_result_persist')) {
              const hookResult = await hooks.run('tool_result_persist', {
                tool: pending.toolCall.name,
                toolCall: pending.toolCall,
                result: pending.result,
              });
              if (hookResult?.result) {
                pending.result = hookResult.result;
              }
            }
            const logInput = privacySettings.redactLogs
              ? redactObject(pending.toolCall.input, privacySettings)
              : pending.toolCall.input;
            const logResult = privacySettings.redactLogs
              ? redactObject(pending.result, privacySettings)
              : pending.result;
            telem.logToolCall(pending.toolCall.name, logInput, logResult, pending.duration);
            emitEvent(onEvent, {
              type: 'tool_execution_end',
              toolCallId: pending.toolCall.id,
              toolName: pending.toolCall.name,
              result: redactEventValue(pending.result),
              isError: Boolean(pending.result?.is_error || pending.result?.isError),
            });
            if (toolUseId) {
              pendingToolCalls.delete(toolUseId);
            }
          }
        }

        if (
          streaming &&
          onPartialMessage &&
          message.type !== 'assistant' &&
          message.type !== 'result' &&
          message.type !== 'user'
        ) {
          onPartialMessage(message);
        }
      }
    } catch (error) {
      if (watchdog?.timedOut && isAbortLikeError(error)) {
        throw watchdog.error || error;
      }
      throw error;
    } finally {
      watchdog?.stop();
    }

    return results;
  };
}
