/**
 * Token-usage counter helpers for the Claude harness.
 *
 * Pure functions extracted from claude-harness.js — no module-scope state.
 */

export const emptyUsageCounters = () => ({
  inputTokens: null,
  outputTokens: null,
  totalTokens: null,
  cacheReadTokens: null,
  cacheWriteTokens: null,
});

export const readUsageCounter = (source, keys) => {
  if (!source || typeof source !== 'object') return null;
  for (const key of keys) {
    const value = source[key];
    if (value === null || value === undefined || value === '') continue;
    const numeric = Number(value);
    if (Number.isFinite(numeric)) {
      return Math.trunc(numeric);
    }
  }
  return null;
};

export const readAnyUsageCounter = (sources, keys) => {
  for (const source of sources) {
    const value = readUsageCounter(source, keys);
    if (value !== null) return value;
  }
  return null;
};

export const mergeUsageCounters = (currentUsage, message) => {
  const nextUsage = currentUsage ? { ...currentUsage } : emptyUsageCounters();
  const direct = message && typeof message === 'object' ? message : null;
  const usageSources = [direct, direct?.usage, direct?.result_usage, direct?.resultUsage];

  const inputTokens = readAnyUsageCounter(usageSources, ['input_tokens', 'inputTokens']);
  const outputTokens = readAnyUsageCounter(usageSources, ['output_tokens', 'outputTokens']);
  const totalTokens = readAnyUsageCounter(usageSources, ['total_tokens', 'totalTokens']);
  const cacheReadTokens = readAnyUsageCounter(usageSources, [
    'cache_read_tokens',
    'cacheReadTokens',
    'cache_read_input_tokens',
    'cacheReadInputTokens',
  ]);
  const cacheWriteTokens = readAnyUsageCounter(usageSources, [
    'cache_write_tokens',
    'cacheWriteTokens',
    'cache_creation_input_tokens',
    'cacheCreationInputTokens',
  ]);

  if (inputTokens !== null) nextUsage.inputTokens = inputTokens;
  if (outputTokens !== null) nextUsage.outputTokens = outputTokens;
  if (totalTokens !== null) nextUsage.totalTokens = totalTokens;
  if (cacheReadTokens !== null) nextUsage.cacheReadTokens = cacheReadTokens;
  if (cacheWriteTokens !== null) nextUsage.cacheWriteTokens = cacheWriteTokens;
  if (
    nextUsage.totalTokens === null &&
    nextUsage.inputTokens !== null &&
    nextUsage.outputTokens !== null
  ) {
    nextUsage.totalTokens = nextUsage.inputTokens + nextUsage.outputTokens;
  }
  return nextUsage;
};

/**
 * Compute total tokens from a usage counter object, falling back to
 * input+output when a total was not reported.
 */
export const computeTotalTokens = (usageCounters) =>
  usageCounters.totalTokens ??
  (usageCounters.inputTokens !== null && usageCounters.outputTokens !== null
    ? usageCounters.inputTokens + usageCounters.outputTokens
    : null);

/** Normalize a cost value to a finite number or null. */
export const normalizeCostUsd = (lastCostUsd) => {
  const normalized =
    lastCostUsd === null || lastCostUsd === undefined || lastCostUsd === ''
      ? null
      : Number(lastCostUsd);
  return Number.isFinite(normalized) ? normalized : null;
};
