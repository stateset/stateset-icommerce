/**
 * Shared option builder for the main `stateset` CLI.
 *
 * Keeping single-request and batch execution on the same builder prevents
 * flag drift between code paths.
 */

export function buildRunAgentLoopOptions({
  request,
  config,
  values,
  treasuryConfig,
  onConfirmRequired,
  resumeSessionId = undefined,
  thinkLevel = 'off',
  providerName = 'claude',
  memoryOverride = null,
  onPartialMessage = null,
  onThinkingBlock = null,
  onToolCall = null,
}) {
  const options = {
    request,
    dbPath: config.db,
    model: config.model,
    allowApply: config.apply,
    agent: values.agent,
    verbose: config.verbose,
    treasury: treasuryConfig,
    onConfirmRequired,
    thinkLevel,
    streaming: Boolean(values.stream),
    maxBudgetUsd: values.budget || null,
    provider: providerName,
    enableMemory: memoryOverride === null ? null : memoryOverride,
    enableX402: values.x402,
  };

  if (resumeSessionId) {
    options.resumeSessionId = resumeSessionId;
  }
  if (onPartialMessage) {
    options.onPartialMessage = onPartialMessage;
  }
  if (onThinkingBlock) {
    options.onThinkingBlock = onThinkingBlock;
  }
  if (onToolCall) {
    options.onToolCall = onToolCall;
  }

  return options;
}
