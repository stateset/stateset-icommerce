import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  resolveTreasuryConfig,
  initTreasuryState,
  createStreamTreasuryChargeRecorder,
} from '../../src/harness/treasury-billing.js';
import { runNonClaudeProvider } from '../../src/harness/provider-run.js';

// ---------------------------------------------------------------------------
// treasury-billing
// ---------------------------------------------------------------------------

describe('harness/treasury-billing resolveTreasuryConfig', () => {
  it('returns null when neither config nor env enables billing', () => {
    assert.equal(resolveTreasuryConfig(null, {}), null);
  });

  it('builds a config from TREASURY_* env vars', () => {
    const config = resolveTreasuryConfig(null, {
      TREASURY_BILLING: 'true',
      TREASURY_CHAIN: 'base',
      TREASURY_TOKEN: 'USDC',
      TREASURY_AGENT: 'agent-1',
      TREASURY_LLM_BILLING: 'false',
    });
    assert.deepEqual(config, {
      enabled: true,
      chainId: 'base',
      tokenSymbol: 'USDC',
      agentId: 'agent-1',
      dbPath: null,
      chargeLlm: false,
    });
  });

  it('fills only missing fields of an explicit config from env', () => {
    const config = resolveTreasuryConfig(
      { enabled: true, chainId: 'solana' },
      {
        TREASURY_CHAIN: 'base',
        TREASURY_TOKEN: 'USDT',
        TREASURY_ERC8004_REGISTRY: '0xreg',
      },
    );
    assert.equal(config.chainId, 'solana', 'explicit value wins');
    assert.equal(config.tokenSymbol, 'USDT');
    assert.equal(config.erc8004Registry, '0xreg');
    assert.equal(config.chargeLlm, true, 'defaults to charging when env unset');
  });
});

const makeRuntime = ({
  balanceSmallest = 100_000_000n,
  token = { symbol: 'USDC', decimals: 6 },
}) => ({
  loadTreasuryContext: async () => ({
    registry: {},
    store: { getBalance: () => ({ balanceSmallest }) },
  }),
  resolveToken: () => token,
  fromSmallestUnit: (smallest, decimals) => (Number(smallest) / 10 ** decimals).toString(),
  getIdentity: () => ({ agent_id: 'erc-agent', agent_wallet: '0xw', owner_address: '0xo' }),
  recordFee: async () => ({
    event_id: 'evt-1',
    amount_display: '0.10',
    amount_smallest: '100000',
    token_symbol: 'USDC',
    chain_id: 'base',
  }),
});

describe('harness/treasury-billing initTreasuryState', () => {
  it('clamps the budget to the wallet balance', async () => {
    const { treasuryState, effectiveBudgetUsd } = await initTreasuryState({
      treasuryConfig: { enabled: true, chainId: 'base', tokenSymbol: 'USDC' },
      dbPath: '/tmp/db',
      maxBudgetUsd: 500,
      runtime: makeRuntime({}),
      includeRequestId: true,
    });
    assert.equal(effectiveBudgetUsd, 100, 'balance (100 USDC) caps the 500 budget');
    assert.equal(treasuryState.enabled, true);
    assert.ok(treasuryState.requestId, 'loop state carries a requestId');
  });

  it('wraps failures as Treasury billing failed and rejects empty balances', async () => {
    await assert.rejects(
      initTreasuryState({
        treasuryConfig: { enabled: true },
        dbPath: '/tmp/db',
        runtime: makeRuntime({ balanceSmallest: 0n }),
      }),
      /Treasury billing failed: Treasury balance is empty/,
    );
    await assert.rejects(
      initTreasuryState({
        treasuryConfig: { enabled: true },
        dbPath: '/tmp/db',
        runtime: { ...makeRuntime({}), resolveToken: () => null },
      }),
      /Treasury billing failed: Unknown treasury token/,
    );
  });
});

describe('harness/treasury-billing createStreamTreasuryChargeRecorder', () => {
  it('records a fee via the state runtime and returns the nested shape', async () => {
    const runtime = makeRuntime({});
    const recorder = createStreamTreasuryChargeRecorder({
      getTreasuryState: () => ({
        enabled: true,
        chargeLlm: true,
        agentId: 'a1',
        chainId: 'base',
        token: { symbol: 'USDC' },
        ctx: {},
        erc8004Identity: null,
        erc8004Registry: null,
        runtime,
      }),
    });
    const charge = await recorder({
      costUsd: 0.1,
      sessionId: 's1',
      provider: 'claude',
      model: 'm',
      usage: null,
    });
    assert.ok(charge.requestId);
    assert.equal(charge.charge.eventId, 'evt-1');
    assert.equal(charge.identity, null);
  });

  it('is a no-op for zero cost or disabled billing', async () => {
    const recorder = createStreamTreasuryChargeRecorder({ getTreasuryState: () => null });
    assert.equal(await recorder({ costUsd: 1, provider: 'p', model: 'm' }), null);

    const disabled = createStreamTreasuryChargeRecorder({
      getTreasuryState: () => ({ enabled: true, chargeLlm: false }),
    });
    assert.equal(await disabled({ costUsd: 1, provider: 'p', model: 'm' }), null);

    const active = createStreamTreasuryChargeRecorder({
      getTreasuryState: () => ({ enabled: true, chargeLlm: true }),
    });
    assert.equal(await active({ costUsd: 0, provider: 'p', model: 'm' }), null);
  });
});

// ---------------------------------------------------------------------------
// provider-run (treasury budget math is exercised through estimateCost)
// ---------------------------------------------------------------------------

describe('harness/provider-run runNonClaudeProvider treasury budget', () => {
  const baseDeps = ({ treasuryState, telemEvents }) => ({
    effectiveProvider: 'unknown-provider-for-test',
    effectiveModel: 'm',
    effectiveThinkLevel: 'off',
    effectiveSlaLevel: null,
    effectiveRequest: 'req',
    requestWithHistory: 'req',
    systemPrompt: 'sys',
    agentName: 'orders',
    routingResult: {},
    promptReport: {},
    streaming: false,
    onMessage: null,
    onPartialMessage: null,
    onEvent: null,
    redactEventText: (t) => t,
    hooks: { hasHooks: () => false },
    privacySettings: {},
    apiKey: null,
    getApiKey: null,
    effectiveSignal: null,
    treasuryState,
    effectiveMaxBudgetUsd: null,
    telem: { logCustomEvent: (type, data) => telemEvents?.push({ type, data }) },
    recordTreasuryLlmCharge: async () => null,
  });

  it('rejects unknown providers with the provider list', async () => {
    await assert.rejects(
      runNonClaudeProvider(baseDeps({ treasuryState: null })),
      /Unknown provider: unknown-provider-for-test/,
    );
  });
});
