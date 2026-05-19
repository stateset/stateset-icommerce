/**
 * Tool-level runtime wrappers — telemetry, audit context, treasury charging,
 * and ERC-8004 identity resolution.
 *
 * Pulled out of mcp-server.js so the orchestrator stays focused on
 * orchestration. Each export is a factory; per-server state (treasury
 * identity cache, telemetry sink) lives inside its closure.
 */

/**
 * Build a `(params, extra) => result` wrapper that times every call and
 * forwards a `logToolCall(toolName, params, result, durationMs)` event to
 * the telemetry sink (or `{ error: message }` on throw, before re-throwing).
 *
 * @param {Object|null} telemetry - optional telemetry sink with `.logToolCall`
 * @returns {(toolName: string, fn: Function) => Function}
 */
export function createWrapWithTelemetry(telemetry) {
  return (toolName, fn) => {
    return async (params, extra) => {
      const startTime = Date.now();
      try {
        const result = await fn(params, extra);
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, result, duration);
        }
        return result;
      } catch (error) {
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, { error: error.message }, duration);
        }
        throw error;
      }
    };
  };
}

/**
 * Build the audit context object recorded with every charged tool call.
 *
 * @param {Object|null} extra - SDK-provided `extra` (carries requestId/sessionId)
 * @param {string} toolName
 */
export function buildAuditContext(extra, toolName) {
  return {
    taskId: extra?.requestId || null,
    requestId: extra?.requestId || null,
    sessionId: extra?.sessionId || null,
    toolName,
  };
}

/**
 * Cache + resolve the ERC-8004 identity (if any) for a treasury agent.
 *
 * Returns three helpers:
 *   - `resolveIdentity()` — load + cache the identity record (or null)
 *   - `getAgentId()` — preferred on-chain agent id, falls back to config
 *   - `getMetadata()` — `{ erc8004: { registry, agentId, wallet, owner } }`
 *
 * @param {Object} options
 * @param {string|null} options.registry - ERC-8004 registry address (or null to disable)
 * @param {string} options.dbPath
 * @param {string} options.agentId - default agent id
 */
export function createTreasuryIdentityResolver({ registry, dbPath, agentId }) {
  let loaded = false;
  let cache = null;

  const resolveIdentity = async () => {
    if (!registry) return null;
    if (loaded) return cache;
    loaded = true;
    try {
      const { getIdentity } = await import('../erc8004/index.js');
      cache = getIdentity(dbPath, registry, agentId);
    } catch {
      cache = null;
    }
    if (!cache) {
      throw new Error(`ERC-8004 identity not found for ${registry}:${agentId}`);
    }
    return cache;
  };

  const getAgentId = async () => {
    const identity = await resolveIdentity();
    return identity?.agent_id || agentId;
  };

  const getMetadata = async () => {
    const identity = await resolveIdentity();
    if (!identity) return {};
    return {
      erc8004: {
        registry,
        agentId: identity.agent_id,
        wallet: identity.agent_wallet,
        owner: identity.owner_address,
      },
    };
  };

  return { resolveIdentity, getAgentId, getMetadata };
}

/**
 * Build the per-server `maybeChargeForTool` gate that integrates treasury
 * billing (pricing rules + ERC-8004 identity + MPP credentials) into the
 * tool dispatch path.
 *
 * @param {Object} deps
 * @param {(() => boolean) | boolean} deps.treasuryEnabled
 * @param {(() => Object) | Object} deps.treasuryContextOptions
 * @param {boolean} deps.allowApply
 * @param {{ getAgentId(): Promise<string>, getMetadata(): Promise<Object> }} deps.identity
 */
export function createToolCharger({
  treasuryEnabled,
  treasuryContextOptions,
  allowApply,
  identity,
}) {
  const isEnabled =
    typeof treasuryEnabled === 'function' ? treasuryEnabled : () => Boolean(treasuryEnabled);
  const getContextOptions =
    typeof treasuryContextOptions === 'function'
      ? treasuryContextOptions
      : () => treasuryContextOptions;

  return async function maybeChargeForTool(
    toolName,
    extra,
    { dryRun = false, allowChargeWrite = false, paymentCredential = null } = {},
  ) {
    if (!isEnabled()) {
      return { charged: false };
    }
    try {
      const { loadTreasuryContext, getToolPricing, resolveToken, recordFee } =
        await import('../treasury/index.js');
      const { toSmallestUnit } = await import('../chains/config.js');
      const ctx = await loadTreasuryContext(getContextOptions());
      const rule = getToolPricing(ctx.pricing, toolName);
      if (!rule) return { charged: false };

      if (!allowApply && !allowChargeWrite) {
        return {
          charged: false,
          blocked: true,
          reason: `Tool ${toolName} requires a treasury charge. Re-run with --apply.`,
        };
      }

      const token = resolveToken(rule.chainId, rule.tokenSymbol, ctx.registry);
      if (!token) {
        return {
          charged: false,
          blocked: true,
          reason: `Unknown token ${rule.tokenSymbol} on ${rule.chainId}.`,
        };
      }
      const amount = Number(rule.amount);
      if (!Number.isFinite(amount) || amount <= 0) {
        return {
          charged: false,
          blocked: true,
          reason: `Invalid pricing amount for ${toolName}.`,
        };
      }
      const effectiveAgentId = await identity.getAgentId();
      const identityMeta = await identity.getMetadata();
      const balance = ctx.store.getBalance({
        agentId: effectiveAgentId,
        chainId: rule.chainId,
        tokenSymbol: token.symbol,
        tokenDecimals: token.decimals,
      });

      const required = toSmallestUnit(amount, token.decimals);

      if (balance.balanceSmallest < required) {
        return {
          charged: false,
          blocked: true,
          reason: `Insufficient ${token.symbol} balance for ${toolName}. Required ${rule.amount} ${token.symbol}.`,
        };
      }

      if (dryRun) {
        return {
          charged: false,
          blocked: false,
          simulated: true,
          rule,
        };
      }

      const audit = buildAuditContext(extra, toolName);
      await recordFee(
        {
          agentId: effectiveAgentId,
          chainId: rule.chainId,
          tokenSymbol: token.symbol,
          amount,
          source: 'task',
          metadata: {
            pricingRule: rule,
            mpp: paymentCredential
              ? {
                  challengeId: paymentCredential.challengeId || null,
                  credentialId: paymentCredential.credentialId || null,
                  paymentMethod: paymentCredential?.method?.kind || null,
                }
              : null,
            ...identityMeta,
          },
          ...audit,
        },
        ctx,
      );

      return {
        charged: true,
        rule,
        mpp: paymentCredential
          ? {
              challengeId: paymentCredential.challengeId || null,
              credentialId: paymentCredential.credentialId || null,
              paymentMethod: paymentCredential?.method?.kind || null,
            }
          : null,
      };
    } catch (error) {
      return { charged: false, blocked: true, reason: error.message };
    }
  };
}
