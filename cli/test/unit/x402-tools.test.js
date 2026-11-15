/**
 * Unit tests for x402 MCP tool handlers.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { x402Tools } from '../../src/tools/x402.js';

function createMockCommerce(overrides = {}) {
  const x402Api = {
    signIntent: async (_intentId, payload) => ({
      id: 'intent_123',
      status: 'signed',
      payer_signature: payload.signature,
      payer_public_key: payload.public_key,
    }),
    ...overrides,
  };
  return {
    x402: () => x402Api,
  };
}

function createGetterStyleCommerce(overrides = {}) {
  return {
    x402: {
      ...overrides,
    },
  };
}

describe('x402_sign_intent tool', () => {
  it('returns preview details when allowApply is false', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_sign_intent');
    assert.ok(tool, 'x402_sign_intent should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: { intentId: 'intent_123' },
      allowApply: false,
    });

    assert.equal(result.error, 'Signing x402 intent requires --apply flag.');
    assert.equal(result.wouldSign.intentId, 'intent_123');
    assert.equal(result.wouldSign.mode, 'local_agent_key');
  });

  it('supports manual signing mode', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_sign_intent');
    assert.ok(tool, 'x402_sign_intent should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: {
        intentId: 'intent_123',
        signature: '0xabc123',
        publicKey: '0xdef456',
      },
      allowApply: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.signing.mode, 'manual_signature');
    assert.equal(result.intent.id, 'intent_123');
  });

  it('rejects mixing local and manual signing inputs', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_sign_intent');
    assert.ok(tool, 'x402_sign_intent should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: {
        intentId: 'intent_123',
        signature: '0xabc123',
        publicKey: '0xdef456',
        agentId: 'agent-payer',
      },
      allowApply: true,
    });

    assert.ok(result.error.includes('either local signing params'));
  });
});

describe('x402_execute_agent_payment tool', () => {
  it('returns preview details when allowApply is false', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_execute_agent_payment');
    assert.ok(tool, 'x402_execute_agent_payment should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: {
        amount: 1000000,
        payeeAddress: '0xabc',
      },
      allowApply: false,
    });

    assert.equal(result.error, 'Executing an end-to-end x402 payment requires --apply flag.');
    assert.equal(result.wouldExecute.amount, 1000000);
    assert.equal(result.wouldExecute.payeeAddress, '0xabc');
    assert.equal(result.wouldExecute.network, 'set_chain');
  });
});

describe('x402_create_payment_intent tool', () => {
  it('supports the raw getter-style x402 binding shape', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_create_payment_intent');
    assert.ok(tool, 'x402_create_payment_intent should exist');

    /** @type {Record<string, unknown> | null} */
    let received = null;
    const commerce = createGetterStyleCommerce({
      createIntent: async (payload) => {
        received = payload;
        return {
          id: 'intent_123',
          status: 'created',
          payerAddress: payload.payerAddress,
          payeeAddress: payload.payeeAddress,
          amount: payload.amount,
          amountDecimal: 1,
          asset: payload.asset,
          network: payload.network,
          chainId: 84532001,
          signingHash: '0xabc123',
          validUntil: 1_700_000_000,
          nonce: 0,
        };
      },
    });

    const result = await tool.handler({
      commerce,
      params: {
        payerAddress: '0xPayer',
        payeeAddress: '0xPayee',
        amount: 1_000_000,
      },
    });

    assert.equal(result.success, true);
    assert.equal(received?.payerAddress, '0xPayer');
    assert.equal(received?.payeeAddress, '0xPayee');
    assert.equal(result.intent.signingHash, '0xabc123');
  });
});

describe('x402_get_credit_account tool', () => {
  it('reads the credit account from the getter-style x402 binding', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_get_credit_account');
    assert.ok(tool, 'x402_get_credit_account should exist');

    const commerce = createGetterStyleCommerce({
      getCreditAccount: async (payload) => ({
        id: 'credit_123',
        payerAddress: payload.payerAddress,
        asset: payload.asset,
        network: payload.network,
        balance: '2500000',
      }),
    });

    const result = await tool.handler({
      commerce,
      params: {
        payerAddress: '0xPayer',
        asset: 'usdc',
        network: 'base',
      },
    });

    assert.equal(result.success, true);
    assert.equal(result.account.id, 'credit_123');
    assert.equal(result.account.payerAddress, '0xPayer');
    assert.equal(result.account.network, 'base');
  });
});

/**
 * The engine's markSettled accepts only a sequenced or batched, unexpired
 * intent. The tool used to accept `signed` too and check expiry not at all,
 * so for a signed intent it executed the payment on-chain and then
 * markSettled threw: money moved, intent left signed. Every refusal must now
 * come before executePayment. A fake chains module stands in for wallets and
 * payment execution, so nothing touches a chain.
 */
describe('x402_settle_intent_onchain refuses before moving money', () => {
  const PAYER = '0x1111111111111111111111111111111111111111';
  const PAYEE = '0x2222222222222222222222222222222222222222';
  const tool = () => x402Tools.find((entry) => entry.name === 'x402_settle_intent_onchain');

  function intent(overrides = {}) {
    return {
      id: 'intent_1',
      status: 'sequenced',
      payer_address: PAYER,
      payee_address: PAYEE,
      amount: '1000000',
      asset: 'usdc',
      network: 'base',
      nonce: 1,
      valid_until: Math.floor(Date.now() / 1000) + 3600,
      ...overrides,
    };
  }

  function fakeChains(executeResult = { success: false, error: 'stop: payment reached' }) {
    const calls = { executePayment: [] };
    return {
      calls,
      module: {
        getChain: (id) => (id === 'base' ? { chainId: 8453 } : null),
        listChains: () => ['base'],
        getToken: () => ({ symbol: 'USDC', decimals: 6 }),
        getDefaultStablecoin: () => ({ symbol: 'USDC', decimals: 6 }),
        getWalletAddress: async () => PAYER,
        isEvmChain: () => true,
        fromSmallestUnit: (value, decimals) => (Number(value) / 10 ** decimals).toFixed(decimals),
        executePayment: async (payment) => {
          calls.executePayment.push(payment);
          return executeResult;
        },
      },
    };
  }

  function commerceWith(storedIntent, { markSettled } = {}) {
    const calls = { markSettled: [] };
    const api = {
      getIntent: async () => storedIntent,
      markSettled: async (...args) => {
        calls.markSettled.push(args);
        if (markSettled) return markSettled(...args);
        return { ...storedIntent, status: 'settled', tx_hash: args[1] };
      },
    };
    return { calls, commerce: { x402: () => api } };
  }

  async function settle(storedIntent, chains, options) {
    const { calls, commerce } = commerceWith(storedIntent, options);
    const result = await tool().handler({
      commerce,
      params: { intentId: 'intent_1', agentId: 'agent-payer' },
      allowApply: true,
      chainsModule: chains.module,
    });
    return { result, markSettledCalls: calls.markSettled };
  }

  for (const status of ['signed', 'created', 'settled', 'expired', 'failed']) {
    it(`refuses an intent in ${status} state without paying, and says which state is required`, async () => {
      const chains = fakeChains();
      const { result, markSettledCalls } = await settle(intent({ status }), chains);

      assert.equal(result.success, false);
      assert.match(result.error, /must be sequenced or batched/);
      assert.match(result.error, new RegExp(`current: ${status}`));
      assert.deepEqual(result.requiredStatus, ['sequenced', 'batched']);
      assert.equal(chains.calls.executePayment.length, 0, 'no payment may be executed');
      assert.equal(markSettledCalls.length, 0);
    });
  }

  it('refuses an expired intent without paying', async () => {
    const chains = fakeChains();
    const validUntil = Math.floor(Date.now() / 1000) - 5;
    const { result } = await settle(intent({ valid_until: validUntil }), chains);

    assert.equal(result.success, false);
    assert.match(result.error, /expired/);
    assert.equal(result.validUntil, validUntil);
    assert.equal(chains.calls.executePayment.length, 0);
  });

  it('refuses an intent with no valid_until without paying', async () => {
    const chains = fakeChains();
    const { result } = await settle(intent({ valid_until: undefined }), chains);

    assert.equal(result.success, false);
    assert.match(result.error, /valid_until/);
    assert.equal(chains.calls.executePayment.length, 0);
  });

  for (const status of ['sequenced', 'batched']) {
    it(`still pays a ${status}, unexpired intent`, async () => {
      const chains = fakeChains();
      const { result } = await settle(intent({ status }), chains);

      assert.equal(chains.calls.executePayment.length, 1, 'the intended path still pays');
      assert.equal(chains.calls.executePayment[0].toAddress, PAYEE);
      assert.equal(result.error, 'stop: payment reached');
    });
  }

  it('reports the transaction when recording the settlement fails after paying', async () => {
    const chains = fakeChains({ success: true, txHash: '0xabc', blockNumber: 42 });
    const { result, markSettledCalls } = await settle(intent(), chains, {
      markSettled: () => {
        throw new Error('Payment intent has expired');
      },
    });

    assert.equal(markSettledCalls.length, 1);
    assert.equal(result.success, false);
    assert.equal(result.paymentExecuted, true);
    assert.equal(result.txHash, '0xabc');
    assert.equal(result.blockNumber, 42);
    assert.match(result.error, /payment was sent \(tx 0xabc\)/);
    assert.match(result.error, /do not retry/);
  });
});
