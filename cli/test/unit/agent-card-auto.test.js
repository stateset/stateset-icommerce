import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createAgentRuntime, makeCommerceProxy } from '../../src/a2a/agent-runtime.js';
import { A2AStore } from '../../src/a2a/store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeTestRuntime(overrides = {}) {
  const store = new A2AStore({ dbPath: ':memory:' });
  store.init();
  const commerce = makeCommerceProxy(store);

  const defaults = {
    name: 'TestAgent',
    walletAddress: '0xTestWallet' + Math.random().toString(16).slice(2, 10),
    signingKey: { privateKey: 'abc', publicKey: 'def' },
    commerce,
    budget: { daily: 500, perTransaction: 100 },
    logger: () => {},
  };

  const runtime = createAgentRuntime({ ...defaults, ...overrides });
  return { runtime, store, commerce };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('agent-card-auto', () => {
  describe('agent_cards CRUD in A2AStore', () => {
    let store;

    beforeEach(() => {
      store = new A2AStore({ dbPath: ':memory:' });
      store.init();
    });

    it('registerAgent creates a card', () => {
      const card = store.registerAgent({
        name: 'Bot1',
        wallet_address: '0xWallet1',
        public_key: 'pk1',
      });
      assert.ok(card);
      assert.strictEqual(card.name, 'Bot1');
      assert.strictEqual(card.wallet_address, '0xWallet1');
      assert.strictEqual(card.trust_level, 'sandbox');
      assert.strictEqual(card.active, 1);
    });

    it('registerAgent stores network-specific payment addresses', () => {
      const card = store.registerAgent({
        name: 'ShieldedBot',
        wallet_address: '0xWalletShielded',
        payment_addresses: {
          bitcoin: 'bc1qshielded',
          zcash: 'u1shielded',
        },
      });

      assert.deepStrictEqual(JSON.parse(card.payment_addresses), {
        bitcoin: 'bc1qshielded',
        zcash: 'u1shielded',
      });
    });

    it('getAgent retrieves by id', () => {
      const card = store.registerAgent({ name: 'Bot2', wallet_address: '0xW2' });
      const fetched = store.getAgent(card.id);
      assert.strictEqual(fetched.id, card.id);
      assert.strictEqual(fetched.name, 'Bot2');
    });

    it('getAgentByWallet retrieves by wallet address', () => {
      store.registerAgent({ name: 'Bot3', wallet_address: '0xW3' });
      const fetched = store.getAgentByWallet('0xW3');
      assert.ok(fetched);
      assert.strictEqual(fetched.name, 'Bot3');
    });

    it('getAgentByWallet returns null for unknown address', () => {
      const result = store.getAgentByWallet('0xUnknown');
      assert.strictEqual(result, null);
    });

    it('listAgents returns all cards', () => {
      store.registerAgent({ name: 'A', wallet_address: '0xA' });
      store.registerAgent({ name: 'B', wallet_address: '0xB' });
      const all = store.listAgents();
      assert.strictEqual(all.length, 2);
    });

    it('listAgents filters by active', () => {
      store.registerAgent({ name: 'Active', wallet_address: '0xAct' });
      store.registerAgent({ name: 'Inactive', wallet_address: '0xInact', active: false });
      const active = store.listAgents({ active: true });
      assert.strictEqual(active.length, 1);
      assert.strictEqual(active[0].name, 'Active');
    });

    it('listAgents filters by trust_level', () => {
      store.registerAgent({ name: 'S', wallet_address: '0xS', trust_level: 'sandbox' });
      store.registerAgent({ name: 'V', wallet_address: '0xV', trust_level: 'verified' });
      const verified = store.listAgents({ trust_level: 'verified' });
      assert.strictEqual(verified.length, 1);
      assert.strictEqual(verified[0].name, 'V');
    });

    it('discoverAgents filters active agents by network', () => {
      store.registerAgent({
        name: 'SetBot',
        wallet_address: '0xSet',
        supported_networks: ['set_chain'],
      });
      store.registerAgent({
        name: 'EthBot',
        wallet_address: '0xEth',
        supported_networks: ['ethereum'],
      });
      const result = store.discoverAgents({ network: 'set_chain' });
      assert.strictEqual(result.length, 1);
      assert.strictEqual(result[0].name, 'SetBot');
    });

    it('discoverAgents filters by skill', () => {
      store.registerAgent({
        name: 'Seller',
        wallet_address: '0xSell',
        a2a_skills: ['sell', 'quote'],
      });
      store.registerAgent({
        name: 'Buyer',
        wallet_address: '0xBuy',
        a2a_skills: ['buy'],
      });
      const sellers = store.discoverAgents({ skill: 'sell' });
      assert.strictEqual(sellers.length, 1);
      assert.strictEqual(sellers[0].name, 'Seller');
    });

    it('verifyAgent sets trust_level to verified', () => {
      const card = store.registerAgent({ name: 'Bot', wallet_address: '0xBot' });
      assert.strictEqual(card.trust_level, 'sandbox');
      const verified = store.verifyAgent(card.id);
      assert.strictEqual(verified.trust_level, 'verified');
    });

    it('updateAgent modifies allowed fields', () => {
      const card = store.registerAgent({ name: 'Old', wallet_address: '0xU' });
      const updated = store.updateAgent(card.id, { name: 'New', description: 'Updated' });
      assert.strictEqual(updated.name, 'New');
      assert.strictEqual(updated.description, 'Updated');
    });

    it('updateAgent rejects disallowed fields', () => {
      const card = store.registerAgent({ name: 'Bot', wallet_address: '0xR' });
      assert.throws(
        () => store.updateAgent(card.id, { id: 'new-id' }),
        /not allowed/,
      );
    });

    it('wallet_address UNIQUE constraint prevents duplicates', () => {
      store.registerAgent({ name: 'Bot1', wallet_address: '0xDup' });
      assert.throws(
        () => store.registerAgent({ name: 'Bot2', wallet_address: '0xDup' }),
        /UNIQUE/,
      );
    });
  });

  describe('makeCommerceProxy x402 integration', () => {
    it('x402() proxies agent card methods', () => {
      const store = new A2AStore({ dbPath: ':memory:' });
      store.init();
      const commerce = makeCommerceProxy(store);

      const x402 = commerce.x402();
      assert.ok(typeof x402.getAgent === 'function');
      assert.ok(typeof x402.getAgentByWallet === 'function');
      assert.ok(typeof x402.registerAgent === 'function');
      assert.ok(typeof x402.listAgents === 'function');
      assert.ok(typeof x402.discoverAgents === 'function');
      assert.ok(typeof x402.verifyAgent === 'function');
      assert.ok(typeof x402.updateAgent === 'function');
    });

    it('x402().registerAgent creates card via proxy', () => {
      const store = new A2AStore({ dbPath: ':memory:' });
      store.init();
      const commerce = makeCommerceProxy(store);

      const card = commerce.x402().registerAgent({
        name: 'ProxyBot',
        wallet_address: '0xProxy',
      });
      assert.ok(card);
      assert.strictEqual(card.name, 'ProxyBot');

      const fetched = commerce.x402().getAgentByWallet('0xProxy');
      assert.strictEqual(fetched.id, card.id);
    });
  });

  describe('autoRegisterCard in createAgentRuntime', () => {
    it('creates card when autoRegisterCard=true', () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      const card = runtime.getAgentCard();
      assert.ok(card);
      assert.strictEqual(card.wallet_address, runtime.walletAddress);
      assert.strictEqual(card.name, runtime.name);
      runtime.destroy();
      store.close();
    });

    it('does not create card when autoRegisterCard=false (default)', () => {
      const { runtime, store } = makeTestRuntime();
      const card = runtime.getAgentCard();
      assert.strictEqual(card, null);
      runtime.destroy();
      store.close();
    });

    it('ensureAgentCard is idempotent', () => {
      // autoRegisterCard=false so first manual call creates the card
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: false });
      const first = runtime.ensureAgentCard();
      const second = runtime.ensureAgentCard();
      assert.strictEqual(first.card.id, second.card.id);
      assert.strictEqual(first.created, true);
      assert.strictEqual(second.created, false);
      runtime.destroy();
      store.close();
    });

    it('checkCardActive returns active for registered card', () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      const result = runtime.checkCardActive();
      assert.strictEqual(result.active, true);
      assert.ok(result.card);
      runtime.destroy();
      store.close();
    });

    it('checkCardActive returns not active for suspended card', () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      const card = runtime.getAgentCard();
      store.updateAgent(card.id, {
        active: 0,
        suspended_at: new Date().toISOString(),
      });
      const result = runtime.checkCardActive();
      assert.strictEqual(result.active, false);
      assert.strictEqual(result.reason, 'suspended');
      runtime.destroy();
      store.close();
    });

    it('checkCardActive returns not active for missing card', () => {
      const { runtime, store } = makeTestRuntime(); // No auto-register
      const result = runtime.checkCardActive();
      assert.strictEqual(result.active, false);
      assert.strictEqual(result.reason, 'card_not_found');
      runtime.destroy();
      store.close();
    });

    it('tick skips cycle when card is suspended', async () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      const card = runtime.getAgentCard();
      store.updateAgent(card.id, {
        active: 0,
        suspended_at: new Date().toISOString(),
      });

      const events = [];
      runtime.on('loop:tick', (data) => events.push(data));

      const processed = await runtime.tick();
      assert.strictEqual(processed, 0);
      assert.strictEqual(events.length, 1);
      assert.strictEqual(events[0].skipped, true);
      assert.strictEqual(events[0].reason, 'suspended');
      runtime.destroy();
      store.close();
    });

    it('emits card:registered when creating card', () => {
      const store = new A2AStore({ dbPath: ':memory:' });
      store.init();
      const commerce = makeCommerceProxy(store);

      const events = [];
      const runtime = createAgentRuntime({
        name: 'EventBot',
        walletAddress: '0xEvt' + Math.random().toString(16).slice(2, 10),
        signingKey: { privateKey: 'a', publicKey: 'b' },
        commerce,
        logger: () => {},
        autoRegisterCard: false,
      });

      runtime.on('card:registered', (data) => events.push(data));
      runtime.ensureAgentCard();
      assert.strictEqual(events.length, 1);
      assert.ok(events[0].card);
      runtime.destroy();
      store.close();
    });

    it('emits card:exists when card already registered', () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      const events = [];
      runtime.on('card:exists', (data) => events.push(data));
      runtime.ensureAgentCard(); // Second call
      assert.strictEqual(events.length, 1);
      runtime.destroy();
      store.close();
    });

    it('respects agentSkills and supportedNetworks options', () => {
      const { runtime, store } = makeTestRuntime({
        autoRegisterCard: true,
        agentSkills: ['analyze', 'predict'],
        supportedNetworks: ['ethereum', 'set_chain'],
        supportedAssets: ['USDC', 'ETH'],
        agentDescription: 'Test bot for analytics',
      });

      const card = runtime.getAgentCard();
      assert.ok(card);
      const skills = JSON.parse(card.a2a_skills);
      assert.deepStrictEqual(skills, ['analyze', 'predict']);
      const networks = JSON.parse(card.supported_networks);
      assert.deepStrictEqual(networks, ['ethereum', 'set_chain']);
      assert.strictEqual(card.description, 'Test bot for analytics');
      runtime.destroy();
      store.close();
    });

    it('syncs settlement payment addresses into the agent card', async () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      runtime.settlement = {
        chainId: 'bitcoin',
        getAddress: async () => 'bc1qruntimecard',
      };

      const card = await runtime.syncAgentCard();

      assert.deepStrictEqual(JSON.parse(card.payment_addresses), {
        bitcoin: 'bc1qruntimecard',
      });
      assert.deepStrictEqual(JSON.parse(card.supported_networks), ['bitcoin']);
      assert.deepStrictEqual(JSON.parse(card.supported_assets), ['BTC']);
      runtime.destroy();
      store.close();
    });

    it('preserves existing payout addresses when syncing a new settlement network', async () => {
      const { runtime, store } = makeTestRuntime({ autoRegisterCard: true });
      const initialCard = runtime.getAgentCard();
      store.updateAgent(initialCard.id, {
        payment_addresses: JSON.stringify({
          zcash: 'u1existingzcash',
        }),
      });

      runtime.settlement = {
        chainId: 'bitcoin',
        getAddress: async () => 'bc1qmergedruntime',
      };

      const card = await runtime.syncAgentCard();

      assert.deepStrictEqual(JSON.parse(card.payment_addresses), {
        zcash: 'u1existingzcash',
        bitcoin: 'bc1qmergedruntime',
      });
      runtime.destroy();
      store.close();
    });
  });
});
