import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import test from 'node:test';

import {
  KernelMarketplaceBridge,
  MemoryBridgeStore,
  SqliteBridgeStore,
  createAwardCommandPlanner,
  signMarketplaceMessage,
  verifyMarketplaceMessage,
} from '../../src/marketplace/kernel-bridge.js';

const TENANT = '10000000-0000-4000-8000-000000000001';
const STORE = '20000000-0000-4000-8000-000000000001';
const BUYER = '30000000-0000-4000-8000-000000000001';
const MERCHANT = '40000000-0000-4000-8000-000000000001';
const EVENT = '50000000-0000-4000-8000-000000000001';

function fixture() {
  const buyerKeys = crypto.generateKeyPairSync('ed25519');
  const merchantKeys = crypto.generateKeyPairSync('ed25519');
  const message = signMarketplaceMessage(
    {
      protocol: 'stateset.marketplace.v1',
      message_id: EVENT,
      conversation_id: 'auction:kernel-bridge-test',
      in_reply_to: null,
      from: 'buyer.acme',
      to: [MERCHANT],
      sent_at: '2026-09-04T12:00:00.000Z',
      expires_at: '2099-09-05T12:00:00.000Z',
      kind: 'award',
      bid_id: 'bid:accepted',
      winner: MERCHANT,
      commitment: {
        amount: { amount: '4550.00', currency: 'USD' },
        counterparty_id: MERCHANT,
        quantity: '50',
        asset: 'SKU-100',
      },
      settlement: {
        asset_amount: { amount: '4550.00', asset: 'USDC' },
        network: 'set_chain',
        buyer_address: 'wallet:buyer',
        seller_address: 'wallet:merchant',
      },
    },
    buyerKeys.privateKey,
    'buyer-key-1',
  );
  const sequenced = {
    sequenceNumber: 1,
    envelope: {
      eventId: EVENT,
      tenantId: TENANT,
      storeId: STORE,
      entityType: 'marketplace.negotiation',
      entityId: 'auction:kernel-bridge-test',
      eventType: 'marketplace.award.created',
      sourceAgent: BUYER,
      createdAt: message.sent_at,
      payload: message,
    },
  };
  const registry = new Map([
    [BUYER, { id: BUYER, name: 'buyer.acme', publicKey: buyerKeys.publicKey }],
    [MERCHANT, { id: MERCHANT, name: 'merchant.beta', publicKey: merchantKeys.publicKey }],
  ]);
  return { buyerKeys, message, sequenced, registry };
}

function sequencerFor(event) {
  return {
    async pull(from) {
      return { events: from <= event.sequenceNumber ? [event] : [], headSequence: 1 };
    },
  };
}

function runtimeOptions(overrides = {}) {
  const data = fixture();
  const calls = [];
  const published = [];
  return {
    data,
    calls,
    published,
    options: {
      id: 'buyer-marketplace-worker',
      sequencer: sequencerFor(data.sequenced),
      commerce: {
        async executeKernelCommand(command, policy) {
          calls.push({ command, policy });
          return {
            receipt_id: 'receipt:1',
            command_id: command.command_id,
            idempotency_key: command.idempotency_key,
            command_type: command.command_type,
            status: 'succeeded',
          };
        },
      },
      identity: {
        id: BUYER,
        principalId: 'company:acme',
        tenantId: TENANT,
        storeId: STORE,
        capabilities: ['a2a.escrow.create'],
      },
      policy: {
        version: 'procurement-authority-v4',
        commands: { 'a2a.escrow.create': {} },
        trusted_authority_keys: {},
      },
      registry: data.registry,
      planner: createAwardCommandPlanner({ side: 'buyer' }),
      async publishReceipt(value) {
        published.push(value);
      },
      ...overrides,
    },
  };
}

test('marketplace messages are signed over canonical content', () => {
  const { buyerKeys, message } = fixture();
  assert.equal(verifyMarketplaceMessage(message, buyerKeys.publicKey), true);
  const rawPublicKey = buyerKeys.publicKey.export({ format: 'der', type: 'spki' }).subarray(-32);
  assert.equal(verifyMarketplaceMessage(message, rawPublicKey.toString('hex')), true);
  assert.equal(
    verifyMarketplaceMessage(
      { ...message, commitment: { ...message.commitment, quantity: '500' } },
      buyerKeys.publicKey,
    ),
    false,
  );
});

test('buyer bridge turns a signed award into a governed escrow command', async () => {
  const { options, calls, published } = runtimeOptions();
  const bridge = new KernelMarketplaceBridge(options);
  const first = await bridge.pollOnce();

  assert.equal(first.outcomes[0].status, 'completed');
  assert.equal(calls.length, 1);
  assert.equal(published.length, 1);
  const command = calls[0].command;
  assert.equal(command.command_type, 'a2a.escrow.create');
  assert.equal(command.mode, 'apply');
  assert.equal(command.principal.id, BUYER);
  assert.deepEqual(command.commitment.asset_amount, { amount: '4550.00', asset: 'USDC' });
  assert.equal(command.commitment.counterparty_id, MERCHANT);
  assert.equal(command.payload.amount, '4550.00');
  assert.equal(command.payload.asset, 'USDC');
  assert.equal(first.nextSequence, 2);

  const second = await bridge.pollOnce();
  assert.equal(second.outcomes.length, 0);
  assert.equal(calls.length, 1);
});

test('merchant bridge derives an inventory reservation, never buyer payment authority', async () => {
  const { data, options, calls } = runtimeOptions({
    id: 'merchant-marketplace-worker',
    identity: {
      id: MERCHANT,
      principalId: 'company:merchant-beta',
      tenantId: TENANT,
      storeId: STORE,
      capabilities: ['inventory.reserve'],
    },
    policy: {
      version: 'merchant-fulfillment-v1',
      commands: { 'inventory.reserve': {} },
      trusted_authority_keys: {},
    },
    planner: createAwardCommandPlanner({ side: 'merchant' }),
  });
  options.registry = data.registry;
  await new KernelMarketplaceBridge(options).pollOnce();
  assert.equal(calls[0].command.command_type, 'inventory.reserve');
  assert.equal(calls[0].command.payload.sku, 'SKU-100');
  assert.equal(calls[0].command.payload.quantity, '50');
  assert.equal(calls[0].command.principal.id, MERCHANT);
});

test('forged awards fail closed before kernel execution and do not advance the cursor', async () => {
  const { options, calls, data } = runtimeOptions();
  data.sequenced.envelope.payload.commitment.quantity = '500';
  const store = new MemoryBridgeStore();
  const bridge = new KernelMarketplaceBridge({
    ...options,
    sequencer: sequencerFor(data.sequenced),
    store,
  });
  await assert.rejects(bridge.pollOnce(), /invalid marketplace message signature/);
  assert.equal(calls.length, 0);
  assert.equal(store.getCursor(options.id), 1);
});

test('a sequencer gap halts before economic execution', async () => {
  const { options, calls, data } = runtimeOptions();
  data.sequenced.sequenceNumber = 2;
  const bridge = new KernelMarketplaceBridge({
    ...options,
    sequencer: {
      async pull() {
        return { events: [data.sequenced], headSequence: 2 };
      },
    },
  });
  await assert.rejects(bridge.pollOnce(), /sequencer gap: expected 1, received 2/);
  assert.equal(calls.length, 0);
});

test('planner output cannot escalate beyond the local trusted principal', async () => {
  const { options, calls } = runtimeOptions({
    planner: async ({ event }) => [
      {
        command_id: EVENT,
        idempotency_key: `sequencer:${event.eventId}:evil`,
        command_type: 'payments.create',
        principal: { id: 'agent:attacker', tenant_id: TENANT },
        store_id: STORE,
        mode: 'apply',
        payload: {},
      },
    ],
  });
  await assert.rejects(
    new KernelMarketplaceBridge(options).pollOnce(),
    /outside the trusted identity scope/,
  );
  assert.equal(calls.length, 0);
});

test('a receipt publication crash retries with the same kernel idempotency key', async () => {
  let attempts = 0;
  const publicationIds = [];
  const { options, calls } = runtimeOptions({
    async publishReceipt({ publicationId }) {
      attempts += 1;
      publicationIds.push(publicationId);
      if (attempts === 1) throw new Error('sequencer unavailable');
    },
  });
  const bridge = new KernelMarketplaceBridge(options);
  await assert.rejects(bridge.pollOnce(), /sequencer unavailable/);
  await bridge.pollOnce();
  assert.equal(calls.length, 2);
  assert.equal(calls[0].command.command_id, calls[1].command.command_id);
  assert.equal(calls[0].command.idempotency_key, calls[1].command.idempotency_key);
  assert.equal(publicationIds[0], publicationIds[1]);
});

test('SQLite bridge state survives worker reconstruction', async (t) => {
  let Database;
  try {
    ({ default: Database } = await import('better-sqlite3'));
  } catch {
    t.skip('better-sqlite3 is optional');
    return;
  }
  const db = new Database(':memory:');
  t.after(() => db.close());
  const { options, calls } = runtimeOptions({ store: new SqliteBridgeStore(db) });
  await new KernelMarketplaceBridge(options).pollOnce();
  const reconstructed = new KernelMarketplaceBridge({
    ...options,
    store: new SqliteBridgeStore(db),
  });
  const result = await reconstructed.pollOnce();
  assert.equal(result.outcomes.length, 0);
  assert.equal(calls.length, 1);
});
