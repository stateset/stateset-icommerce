/**
 * Tool runtime metadata + agentic pricing helpers.
 *
 * Pulled out of mcp-server.js so the orchestrator stays focused on
 * orchestration. The pure helpers (`buildToolRuntimeMeta`) take their
 * dependency map as an argument; the stateful cache (`createPricingCache`)
 * is a factory so per-server state stays per-server.
 */

/**
 * Build the runtime metadata bundle for a tool — name, permission, policy
 * domain, side-effect classification, idempotency, and compensation hints.
 *
 * Pure function: same inputs always produce the same shape.
 *
 * @param {string} toolName
 * @param {Object} deps
 * @param {Map<string, Object>} deps.toolDefsByName
 * @param {(toolName: string) => string} deps.inferPolicyDomain
 * @param {Record<string, string>} deps.toolDomainByName
 * @param {Record<string, Array<unknown>>} deps.compensationHints
 * @param {Set<string>} deps.idempotencyHints
 */
export function buildToolRuntimeMeta(
  toolName,
  { toolDefsByName, inferPolicyDomain, toolDomainByName, compensationHints, idempotencyHints },
) {
  const candidate = toolDefsByName.get(toolName);
  if (!candidate) {
    return {
      name: toolName,
      permission: 'unknown',
      policyDomain: inferPolicyDomain(toolName),
      sideEffect: 'unknown',
      compensations: [],
      idempotent: false,
    };
  }
  const permission = candidate?.permission || 'unknown';
  return {
    name: candidate.name,
    permission,
    policyDomain:
      candidate?.policyDomain || toolDomainByName[toolName] || inferPolicyDomain(toolName),
    sideEffect: permission === 'read' ? 'read' : 'write',
    description: candidate.description || '',
    compensations: compensationHints[toolName] || [],
    idempotent: idempotencyHints.has(toolName),
    replay: {
      paramsHash: true,
      resultHash: true,
    },
  };
}

/**
 * Factory that owns the per-server pricing state cache and exposes
 * `loadState()` and `getPricing(toolName)`. Treasury modules are
 * dynamically imported so this module stays cheap to load when treasury
 * is disabled.
 *
 * The treasury config is resolved lazily via getters so this factory can
 * be wired before the caller has finished computing its treasury settings —
 * keeps mcp-server.js init ordering flexible without forcing a hoist.
 *
 * @param {Object} options
 * @param {(() => boolean) | boolean} options.treasuryEnabled - bool or getter
 * @param {(() => Object) | Object} options.treasuryContextOptions - obj or getter
 */
export function createPricingCache({ treasuryEnabled, treasuryContextOptions }) {
  const isEnabled =
    typeof treasuryEnabled === 'function' ? treasuryEnabled : () => Boolean(treasuryEnabled);
  const getContextOptions =
    typeof treasuryContextOptions === 'function'
      ? treasuryContextOptions
      : () => treasuryContextOptions;
  let cache = null;

  const loadState = async () => {
    if (cache !== null) return cache;
    if (!isEnabled()) {
      cache = { loaded: false, disabled: true };
      return cache;
    }
    try {
      const { loadTreasuryContext } = await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(getContextOptions());
      cache = {
        loaded: true,
        pricing: ctx.pricing,
        registry: ctx.registry,
        loadedAt: new Date().toISOString(),
      };
    } catch (error) {
      cache = { loaded: false, error: error.message };
    }
    return cache;
  };

  const getPricing = async (toolName) => {
    const state = await loadState();
    if (!state?.loaded || !state.pricing || !toolName) {
      return null;
    }
    try {
      const { getToolPricing, resolveToken, toSmallestUnit } = await import('../treasury/index.js');
      const rule = getToolPricing(state.pricing, toolName);
      if (!rule) return null;
      const token = resolveToken(rule.chainId, rule.tokenSymbol, state.registry);
      if (!token) return null;
      const amount = Number(rule.amount);
      const amountSmallest = toSmallestUnit(amount, token.decimals);
      return {
        enabled: true,
        chainId: rule.chainId,
        tokenSymbol: rule.tokenSymbol,
        amount,
        amountSmallest: amountSmallest?.toString?.() || amountSmallest,
        token: {
          symbol: token.symbol,
          chainId: token.chainId,
          address: token.address || null,
          decimals: token.decimals,
        },
      };
    } catch {
      return null;
    }
  };

  return { loadState, getPricing };
}
