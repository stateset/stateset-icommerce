/**
 * Treasury billing helpers for the Claude harness.
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected;
 * there is no module-scope state. Dynamic imports of the treasury/chains/erc8004
 * modules are preserved so the modules stay lazy-loaded exactly as before.
 */

import { randomUUID } from 'node:crypto';

/**
 * Resolve the effective treasury configuration from an explicit config or
 * the TREASURY_* environment variables. Returns null when billing is not
 * requested. Mirrors the original inline env-merge logic exactly.
 */
export function resolveTreasuryConfig(treasury, env = process.env) {
  const envTreasuryEnabled = env.TREASURY_BILLING === 'true';
  const envTreasuryChain = env.TREASURY_CHAIN || null;
  const envTreasuryToken = env.TREASURY_TOKEN || null;
  const envTreasuryAgent = env.TREASURY_AGENT || 'default';
  const envTreasuryDb = env.TREASURY_DB || null;
  const envTreasuryLlm = env.TREASURY_LLM_BILLING === 'true';
  const envTreasuryRegistry = env.TREASURY_ERC8004_REGISTRY || null;
  const envTreasuryRegistryDb = env.TREASURY_ERC8004_DB || null;

  const treasuryConfig = treasury
    ? { ...treasury }
    : envTreasuryEnabled
      ? {
          enabled: true,
          chainId: envTreasuryChain || 'set_chain',
          tokenSymbol: envTreasuryToken || 'USDC',
          agentId: envTreasuryAgent,
          dbPath: envTreasuryDb,
          chargeLlm: env.TREASURY_LLM_BILLING === undefined ? true : envTreasuryLlm,
        }
      : null;

  if (treasuryConfig) {
    if (!treasuryConfig.chainId && envTreasuryChain) {
      treasuryConfig.chainId = envTreasuryChain;
    }
    if (!treasuryConfig.tokenSymbol && envTreasuryToken) {
      treasuryConfig.tokenSymbol = envTreasuryToken;
    }
    if (!treasuryConfig.agentId && envTreasuryAgent) {
      treasuryConfig.agentId = envTreasuryAgent;
    }
    if (!treasuryConfig.dbPath && envTreasuryDb) {
      treasuryConfig.dbPath = envTreasuryDb;
    }
    if (treasuryConfig.chargeLlm === undefined) {
      treasuryConfig.chargeLlm = env.TREASURY_LLM_BILLING === undefined ? true : envTreasuryLlm;
    }
    if (!treasuryConfig.erc8004Registry && envTreasuryRegistry) {
      treasuryConfig.erc8004Registry = envTreasuryRegistry;
    }
    if (!treasuryConfig.erc8004DbPath && envTreasuryRegistryDb) {
      treasuryConfig.erc8004DbPath = envTreasuryRegistryDb;
    }
  }

  return treasuryConfig;
}

/**
 * Default treasury runtime loader (dynamic imports). Used when no
 * treasuryRuntime override is supplied.
 */
export async function loadDefaultTreasuryRuntime() {
  const treasuryModule = await import('../treasury/index.js');
  const chainsModule = await import('../chains/config.js');
  const erc8004Module = await import('../erc8004/index.js');
  return {
    loadTreasuryContext: treasuryModule.loadTreasuryContext,
    resolveToken: treasuryModule.resolveToken,
    recordFee: treasuryModule.recordFee,
    fromSmallestUnit: chainsModule.fromSmallestUnit,
    getIdentity: erc8004Module.getIdentity,
  };
}

/**
 * Shared treasury state initialization: resolves identity, token and balance
 * and computes the effective budget. Throws `Treasury billing failed: ...`
 * on any failure, matching the original behavior.
 *
 * @returns {{ treasuryState: object, effectiveBudgetUsd: number }}
 */
export async function initTreasuryState({
  treasuryConfig,
  dbPath,
  maxBudgetUsd = null,
  runtime,
  includeRequestId = false,
  includeRuntimeInState = false,
}) {
  try {
    const ctx = await runtime.loadTreasuryContext({
      dbPath: treasuryConfig.dbPath || undefined,
    });
    const chainId = treasuryConfig.chainId || 'set_chain';
    const tokenSymbol = treasuryConfig.tokenSymbol || 'USDC';
    let agentId = treasuryConfig.agentId || 'default';
    let erc8004Identity = null;
    const erc8004Registry = treasuryConfig.erc8004Registry || null;
    if (erc8004Registry) {
      const identityDbPath = treasuryConfig.erc8004DbPath || dbPath;
      erc8004Identity = runtime.getIdentity(identityDbPath, erc8004Registry, agentId);
      if (!erc8004Identity) {
        throw new Error(`ERC-8004 identity not found for ${erc8004Registry}:${agentId}`);
      }
      agentId = erc8004Identity.agent_id;
    }
    const token = runtime.resolveToken(chainId, tokenSymbol, ctx.registry);
    if (!token) {
      throw new Error(`Unknown treasury token ${tokenSymbol} on ${chainId}.`);
    }
    const balance = ctx.store.getBalance({
      agentId,
      chainId,
      tokenSymbol: token.symbol,
      tokenDecimals: token.decimals,
    });
    const balanceDisplay = runtime.fromSmallestUnit(balance.balanceSmallest, token.decimals);
    const balanceUsd = Number.parseFloat(balanceDisplay);
    if (!Number.isFinite(balanceUsd) || balanceUsd <= 0) {
      throw new Error(`Treasury balance is empty for ${token.symbol} on ${chainId}.`);
    }
    const resolvedBudget = maxBudgetUsd ? Math.min(Number(maxBudgetUsd), balanceUsd) : balanceUsd;
    if (!Number.isFinite(resolvedBudget) || resolvedBudget <= 0) {
      throw new Error(`Treasury budget unavailable for ${token.symbol} on ${chainId}.`);
    }
    const treasuryState = {
      enabled: true,
      chargeLlm: treasuryConfig.chargeLlm !== false,
      ctx,
      agentId,
      chainId,
      token,
      balanceUsd,
      erc8004Registry,
      erc8004Identity,
      ...(includeRequestId ? { requestId: randomUUID() } : {}),
      ...(includeRuntimeInState ? { runtime } : {}),
    };
    return { treasuryState, effectiveBudgetUsd: resolvedBudget };
  } catch (error) {
    throw new Error(`Treasury billing failed: ${error.message}`);
  }
}

/**
 * Build the erc8004 metadata fragment attached to treasury fee records.
 */
function buildErc8004Meta(treasuryState) {
  return treasuryState.erc8004Identity
    ? {
        erc8004: {
          registry: treasuryState.erc8004Registry,
          agentId: treasuryState.erc8004Identity.agent_id,
          wallet: treasuryState.erc8004Identity.agent_wallet,
          owner: treasuryState.erc8004Identity.owner_address,
        },
      }
    : {};
}

/**
 * Create the LLM charge recorder used by runAgentLoop. Emits telemetry
 * events and returns the flat charge shape (or null on failure/no-op).
 */
export function createLoopTreasuryChargeRecorder({ getTreasuryState, telem }) {
  return async ({ costUsd, sessionId: chargeSessionId, provider, model, usage }) => {
    const treasuryState = getTreasuryState();
    if (!treasuryState?.enabled || !treasuryState.chargeLlm) return null;
    const amount = Number(costUsd);
    if (!Number.isFinite(amount) || amount <= 0) return null;
    try {
      const { recordFee } = await import('../treasury/index.js');
      const erc8004Meta = buildErc8004Meta(treasuryState);
      const entry = await recordFee(
        {
          agentId: treasuryState.agentId,
          chainId: treasuryState.chainId,
          tokenSymbol: treasuryState.token.symbol,
          amount,
          source: 'llm',
          metadata: {
            provider,
            model,
            usage: usage || null,
            costUsd: amount,
            ...erc8004Meta,
          },
          taskId: treasuryState.requestId,
          sessionId: chargeSessionId || null,
          toolName: 'llm_inference',
          requestId: treasuryState.requestId,
        },
        treasuryState.ctx,
      );
      telem.logCustomEvent('treasury_llm_charge', {
        amount,
        token: treasuryState.token.symbol,
        chainId: treasuryState.chainId,
        provider,
        model,
        sessionId: chargeSessionId || null,
        requestId: treasuryState.requestId,
      });
      return {
        eventId: entry.event_id,
        amount: entry.amount_display,
        amountSmallest: entry.amount_smallest,
        token: entry.token_symbol,
        chainId: entry.chain_id,
      };
    } catch (err) {
      telem.logCustomEvent('treasury_llm_charge_failed', {
        error: err.message,
        amount,
        provider,
        model,
      });
      return null;
    }
  };
}

/**
 * Create the LLM charge recorder used by createAgentStreamSession. Uses the
 * runtime stored on treasuryState, mints a fresh requestId per charge and
 * returns the nested { requestId, charge, identity } shape (or null).
 */
export function createStreamTreasuryChargeRecorder({ getTreasuryState }) {
  return async ({ costUsd, sessionId: chargeSessionId, provider, model, usage }) => {
    const treasuryState = getTreasuryState();
    if (!treasuryState?.enabled || !treasuryState.chargeLlm) return null;
    const amount = Number(costUsd);
    if (!Number.isFinite(amount) || amount <= 0) return null;
    try {
      const requestId = randomUUID();
      const erc8004Meta = buildErc8004Meta(treasuryState);
      const entry = await treasuryState.runtime.recordFee(
        {
          agentId: treasuryState.agentId,
          chainId: treasuryState.chainId,
          tokenSymbol: treasuryState.token.symbol,
          amount,
          source: 'llm',
          metadata: {
            provider,
            model,
            usage: usage || null,
            costUsd: amount,
            ...erc8004Meta,
          },
          taskId: requestId,
          sessionId: chargeSessionId || null,
          toolName: 'llm_inference',
          requestId,
        },
        treasuryState.ctx,
      );
      return {
        requestId,
        charge: {
          eventId: entry.event_id,
          amount: entry.amount_display,
          amountSmallest: entry.amount_smallest,
          token: entry.token_symbol,
          chainId: entry.chain_id,
        },
        identity: treasuryState.erc8004Identity,
      };
    } catch (err) {
      console.warn('[Harness] Treasury charge failed:', err.message);
      return null;
    }
  };
}
