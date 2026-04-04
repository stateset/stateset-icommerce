import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { adaptCommerceApis } from '../../src/commerce.js';
import { createA2AService } from '../../src/a2a/index.js';
import { createAgentRuntime, makeCommerceProxy } from '../../src/a2a/agent-runtime.js';
import { A2AStore } from '../../src/a2a/store.js';

const stores = [];

afterEach(() => {
  while (stores.length > 0) {
    const store = stores.pop();
    try {
      store.close();
    } catch {
      // Best effort cleanup for in-memory stores.
    }
  }
});

function wallet(prefix = '0x') {
  return `${prefix}${crypto.randomBytes(20).toString('hex')}`;
}

function signingKey() {
  return {
    privateKey: crypto.randomBytes(32).toString('hex'),
    publicKey: crypto.randomBytes(32).toString('hex'),
  };
}

function createStore() {
  const store = new A2AStore({ dbPath: ':memory:' });
  store.init();
  stores.push(store);
  return store;
}

function createGetterStyleCommerce(store) {
  const base = makeCommerceProxy(store);
  const raw = {};

  Object.defineProperty(raw, 'a2a', {
    enumerable: true,
    configurable: true,
    get() {
      return base.a2a();
    },
  });

  Object.defineProperty(raw, 'x402', {
    enumerable: true,
    configurable: true,
    get() {
      return base.x402();
    },
  });

  return raw;
}

describe('commerce compatibility adapters', () => {
  it('adapts getter and function API surfaces into a single callable shape', async () => {
    const a2aApi = {
      listPayments() {
        return ['payment-1'];
      },
    };
    const x402Api = {
      async createIntent(payload) {
        return { id: 'intent-1', ...payload };
      },
    };
    const raw = {
      a2a() {
        return a2aApi;
      },
    };
    Object.defineProperty(raw, 'x402', {
      enumerable: true,
      configurable: true,
      get() {
        return x402Api;
      },
    });

    const commerce = adaptCommerceApis(raw, ['a2a', 'x402']);

    assert.equal(typeof commerce.a2a, 'function');
    assert.equal(typeof commerce.x402, 'function');
    assert.strictEqual(commerce.a2a(), a2aApi);
    assert.strictEqual(commerce.x402(), x402Api);
    assert.deepEqual(commerce.a2a.listPayments(), ['payment-1']);
    assert.deepEqual(await commerce.x402.createIntent({ amount: 42 }), {
      id: 'intent-1',
      amount: 42,
    });
  });

  it('lets createA2AService pay a registered agent through getter-style commerce APIs', async () => {
    const store = createStore();
    const sellerWallet = wallet();
    store.registerAgent({
      id: crypto.randomUUID(),
      name: 'Getter Seller',
      wallet_address: sellerWallet,
      public_key: signingKey().publicKey,
      supported_networks: ['bitcoin'],
      supported_assets: ['BTC'],
      payment_addresses: { bitcoin: 'bc1qgettercompatrecipient' },
    });

    const service = createA2AService(createGetterStyleCommerce(store), {
      agentId: crypto.randomUUID(),
      walletAddress: wallet(),
    });

    const result = await service.pay({
      to: sellerWallet,
      amount: 0.001,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'getter-style payment',
    });

    assert.equal(result.success, true);
    assert.equal(result.payment.to, 'bc1qgettercompatrecipient');

    const stored = store.getPayment(result.payment.id);
    assert.equal(stored.recipient_address, 'bc1qgettercompatrecipient');
    assert.equal(stored.asset, 'BTC');
    assert.equal(stored.network, 'bitcoin');
  });

  it('lets createAgentRuntime auto-register agent cards through getter-style commerce APIs', () => {
    const store = createStore();
    const runtime = createAgentRuntime({
      name: 'Getter Runtime',
      walletAddress: wallet(),
      signingKey: signingKey(),
      commerce: createGetterStyleCommerce(store),
      budget: { daily: 1000, perTransaction: 1000 },
      autoRegisterCard: true,
      logger: () => {},
    });

    const card = runtime.getAgentCard();

    assert.ok(card);
    assert.equal(card.wallet_address, runtime.walletAddress);
    assert.equal(store.getAgentByWallet(runtime.walletAddress)?.id, runtime.agentId);

    runtime.destroy?.();
  });
});
