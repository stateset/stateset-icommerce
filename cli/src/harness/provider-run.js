/**
 * Non-Claude provider execution path for runAgentLoop.
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected;
 * there is no module-scope state. Behavior (including treasury max-token
 * budgeting via binary search and event emission) is preserved exactly.
 */

import { emitEvent } from '../harness-utils.js';
import { redactSensitive } from '../privacy.js';
import { estimateTokensFromText } from '../conversation-history.js';

export async function runNonClaudeProvider({
  effectiveProvider,
  effectiveModel,
  effectiveThinkLevel,
  effectiveSlaLevel,
  effectiveRequest,
  requestWithHistory,
  systemPrompt,
  agentName,
  routingResult,
  promptReport,
  streaming,
  onMessage,
  onPartialMessage,
  onEvent,
  redactEventText,
  hooks,
  privacySettings,
  apiKey,
  getApiKey,
  effectiveSignal,
  treasuryState,
  effectiveMaxBudgetUsd,
  telem,
  recordTreasuryLlmCharge,
}) {
  const { ensureProviderRegistry } = await import('../providers/base.js');
  const providerRegistry = await ensureProviderRegistry();
  const providerInstance = providerRegistry.get(effectiveProvider);
  if (!providerInstance) {
    throw new Error(
      `Unknown provider: ${effectiveProvider}. Available: ${providerRegistry.list().join(', ')}`,
    );
  }
  if (!(await providerInstance.isAvailable())) {
    const providerConfig = (await import('../config.js')).PROVIDERS[effectiveProvider];
    throw new Error(
      `Provider "${effectiveProvider}" is not available. ${providerConfig?.envKey ? `Set ${providerConfig.envKey} environment variable.` : ''}`,
    );
  }
  const messages = [
    { role: 'system', content: systemPrompt },
    { role: 'user', content: requestWithHistory },
  ];
  let providerApiKey = apiKey;
  if (!providerApiKey && typeof getApiKey === 'function') {
    providerApiKey = await getApiKey(effectiveProvider);
  }
  let providerMaxTokens = null;
  let treasuryBudgetUsd = null;
  if (treasuryState?.enabled) {
    treasuryBudgetUsd = effectiveMaxBudgetUsd ?? treasuryState.balanceUsd;
    if (!Number.isFinite(treasuryBudgetUsd) || treasuryBudgetUsd <= 0) {
      throw new Error(
        `Treasury balance is empty for ${treasuryState.token.symbol} on ${treasuryState.chainId}.`,
      );
    }

    if (typeof providerInstance.estimateCost === 'function') {
      const inputEstimate =
        estimateTokensFromText(systemPrompt) + estimateTokensFromText(requestWithHistory);
      const inputTokensEstimate = Math.ceil(inputEstimate * 1.3) + 32;
      const safetyBudget = treasuryBudgetUsd * 0.95;
      const defaultMaxTokens = 4096;

      const estimateCost = (outputTokens) =>
        providerInstance.estimateCost(
          { inputTokens: inputTokensEstimate, outputTokens },
          effectiveModel,
        );

      const baseCost = estimateCost(0);
      if (baseCost === null || baseCost === undefined) {
        telem.logCustomEvent('treasury_llm_estimate_unavailable', {
          provider: effectiveProvider,
          model: effectiveModel,
        });
      } else if (baseCost > safetyBudget) {
        throw new Error(
          `Treasury balance ${treasuryBudgetUsd.toFixed(4)} is insufficient for estimated input tokens.`,
        );
      } else {
        let low = 0;
        let high = defaultMaxTokens;
        let best = 0;
        while (low <= high) {
          const mid = Math.floor((low + high) / 2);
          const cost = estimateCost(mid);
          if (cost === null || cost === undefined) break;
          if (cost <= safetyBudget) {
            best = mid;
            low = mid + 1;
          } else {
            high = mid - 1;
          }
        }
        if (best <= 0) {
          const oneTokenCost = estimateCost(1);
          if (oneTokenCost !== null && oneTokenCost !== undefined && oneTokenCost <= safetyBudget) {
            providerMaxTokens = 1;
          } else {
            throw new Error(
              `Treasury balance ${treasuryBudgetUsd.toFixed(4)} is insufficient for any output tokens.`,
            );
          }
        } else {
          providerMaxTokens = best;
        }
      }
    } else {
      telem.logCustomEvent('treasury_llm_estimate_unavailable', {
        provider: effectiveProvider,
        model: effectiveModel,
      });
    }
  }
  let assistantStarted = false;
  let partialText = '';
  const handlePartialMessage = (data) => {
    if (!assistantStarted) {
      assistantStarted = true;
      emitEvent(onEvent, {
        type: 'message_start',
        message: { role: 'assistant', content: '' },
      });
    }
    if (data?.text) {
      partialText += data.text;
      emitEvent(onEvent, {
        type: 'message_update',
        message: { role: 'assistant', content: redactEventText(partialText) },
        delta: redactEventText(data.text),
      });
    }
    if (onPartialMessage) onPartialMessage(data);
  };

  const providerResult = await providerInstance.chat(messages, {
    model: effectiveModel,
    stream: streaming,
    onPartialMessage: streaming ? handlePartialMessage : onPartialMessage,
    apiKey: providerApiKey,
    signal: effectiveSignal,
    ...(providerMaxTokens ? { maxTokens: providerMaxTokens } : {}),
  });
  let providerResponse = providerResult.text;
  let budgetExceeded = false;
  if (
    treasuryState?.enabled &&
    providerResult.cost !== null &&
    providerResult.cost !== undefined &&
    treasuryBudgetUsd !== null &&
    treasuryBudgetUsd !== undefined
  ) {
    budgetExceeded = providerResult.cost > treasuryBudgetUsd;
  }
  if (hooks?.hasHooks?.('before_send')) {
    const hookResult = await hooks.run('before_send', {
      request: effectiveRequest,
      response: providerResponse,
      agent: agentName,
      slaLevel: effectiveSlaLevel,
      model: effectiveModel,
      provider: effectiveProvider,
      toolResults: [],
    });
    if (hookResult?.response) {
      providerResponse = hookResult.response;
    }
  }
  const responseForUser = privacySettings.redactResponse
    ? redactSensitive(providerResponse, privacySettings)
    : providerResponse;
  if (onMessage) {
    onMessage(responseForUser);
  }
  if (!assistantStarted) {
    emitEvent(onEvent, {
      type: 'message_start',
      message: { role: 'assistant', content: '' },
    });
  }
  emitEvent(onEvent, {
    type: 'message_end',
    message: { role: 'assistant', content: redactEventText(providerResponse) },
  });
  emitEvent(onEvent, {
    type: 'turn_end',
    response: redactEventText(providerResponse),
    toolResults: [],
  });
  emitEvent(onEvent, {
    type: 'agent_end',
    response: redactEventText(providerResponse),
    toolResults: [],
    sessionId: null,
    agent: agentName,
    slaLevel: effectiveSlaLevel,
    provider: effectiveProvider,
    model: effectiveModel,
    cost: providerResult.cost || null,
    budgetExceeded,
  });
  const treasuryCharge = await recordTreasuryLlmCharge({
    costUsd: providerResult.cost,
    sessionId: null,
    provider: effectiveProvider,
    model: providerResult.model || effectiveModel,
    usage: providerResult.usage,
  });
  return {
    response: responseForUser,
    toolResults: [],
    sessionId: null,
    agent: agentName,
    slaLevel: effectiveSlaLevel,
    routing: routingResult,
    provider: effectiveProvider,
    cost: providerResult.cost || null,
    thinkLevel: effectiveThinkLevel,
    budgetExceeded,
    promptReport,
    treasury: treasuryState
      ? {
          requestId: treasuryState.requestId,
          charge: treasuryCharge,
          identity: treasuryState.erc8004Identity,
        }
      : undefined,
  };
}
