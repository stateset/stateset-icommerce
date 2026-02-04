const THINK_LEVELS = new Set(['off', 'low', 'medium', 'high']);

export function resolveAgentRuntimeOptions(values = {}) {
  const thinkLevel = values.think || 'off';
  if (!THINK_LEVELS.has(thinkLevel)) {
    throw new Error(`Invalid think level '${thinkLevel}'. Use: off, low, medium, high`);
  }

  const providerName = values.provider || 'claude';
  const memoryOverride = values.noMemory ? false : (values.memory ? true : null);

  return {
    thinkLevel,
    providerName,
    streaming: !!values.stream,
    maxBudgetUsd: values.budget || null,
    memoryOverride,
    enableX402: !!values.x402
  };
}

export function createStreamingHandler(enabled) {
  if (!enabled) return null;
  return (event) => {
    if (event?.content) {
      process.stdout.write(event.content);
      return;
    }
    if (event?.delta?.text) {
      process.stdout.write(event.delta.text);
      return;
    }
    if (typeof event?.text === 'string') {
      process.stdout.write(event.text);
    }
  };
}
