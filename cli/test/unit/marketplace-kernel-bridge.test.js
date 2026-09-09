import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import test from 'node:test';

import {
  KernelMarketplaceBridge,
  MemoryBridgeStore,
  SqliteBridgeStore,
  canonicalMarketplaceMessage,
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

test('bridge and purchase runtime share one canonicalizer that rejects undefined', async () => {
  // Imported the way the bridge imports it: through the published subpath, so
  // this test fails if the export map ever stops shipping the module.
  const { canonicalJson } = await import('@stateset/embedded/canonical-json');
  const { canonicalJson: runtimeCanonical } = await import(
    '../../../bindings/node/purchase-runtime.mjs'
  );
  assert.equal(runtimeCanonical, canonicalJson);
  assert.equal(canonicalMarketplaceMessage({ b: 1, a: 2 }), '{"a":2,"b":1}');
  // The literal string `undefined` must never be signable as a missing value.
  for (const canonicalize of [canonicalJson, runtimeCanonical]) {
    assert.throws(() => canonicalize({ winner: undefined }), /JSON serializable/);
    assert.throws(() => canonicalize([undefined]), /JSON serializable/);
  }
  assert.throws(
    () => canonicalMarketplaceMessage({ kind: 'award', winner: undefined }),
    /JSON serializable/,
  );
  const keys = crypto.generateKeyPairSync('ed25519');
  assert.throws(
    () => signMarketplaceMessage({ kind: 'award', winner: undefined }, keys.privateKey),
    /JSON serializable/,
  );
});

/** Re-sign an award after mutating it so the envelope binding still holds.
 * Returns a fresh sequenced event; the fixture stays pristine. */
function repoisoned(data, mutate, sequenceNumber = 1) {
  const payload = structuredClone(data.sequenced.envelope.payload);
  delete payload.signature;
  mutate(payload);
  return {
    sequenceNumber,
    envelope: {
      ...structuredClone(data.sequenced.envelope),
      payload: signMarketplaceMessage(payload, data.buyerKeys.privateKey, 'buyer-key-1'),
    },
  };
}

/** A second, valid award at sequence 2 so we can prove the cursor moves past
 * a poisoned message instead of stalling the whole bridge on it. */
function followingAward(data, sequenceNumber = 2) {
  const eventId = '50000000-0000-4000-8000-000000000002';
  const payload = structuredClone(data.sequenced.envelope.payload);
  delete payload.signature;
  payload.message_id = eventId;
  const envelope = {
    ...structuredClone(data.sequenced.envelope),
    eventId,
    payload: signMarketplaceMessage(payload, data.buyerKeys.privateKey, 'buyer-key-1'),
  };
  return { sequenceNumber, envelope };
}

function sequencerForAll(events) {
  return {
    async pull(from) {
      return {
        events: events.filter((event) => event.sequenceNumber >= from),
        headSequence: events.length,
      };
    },
  };
}

test('a poisoned award is dead-lettered after bounded retries and the queue drains', async () => {
  const { options, calls, data } = runtimeOptions();
  // A zero commitment fails the planner's exact-decimal check on every attempt:
  // a permanently poisoned message that used to stall the cursor forever.
  const poisoned = repoisoned(data, (payload) => {
    payload.commitment.amount.amount = '0';
  });
  const healthy = followingAward(data);
  const store = new MemoryBridgeStore();
  const bridge = new KernelMarketplaceBridge({
    ...options,
    sequencer: sequencerForAll([poisoned, healthy]),
    store,
    maxAttempts: 3,
  });

  await assert.rejects(bridge.pollOnce(), /positive exact decimal/);
  await assert.rejects(bridge.pollOnce(), /positive exact decimal/);
  assert.equal(store.getCursor(options.id), 1, 'cursor holds while retries remain');
  assert.equal(calls.length, 0);

  const drained = await bridge.pollOnce();
  assert.equal(drained.outcomes[0].status, 'dead_lettered');
  assert.equal(drained.outcomes[1].status, 'completed');
  assert.equal(drained.nextSequence, 3);
  assert.equal(calls.length, 1, 'only the healthy award reached the kernel');

  // The poisoned row is retained for operator inspection, not deleted.
  const record = store.record(options.id, EVENT);
  assert.equal(record.status, 'dead_lettered');
  assert.equal(record.attempts, 3);
  assert.match(record.error, /positive exact decimal/);

  // A dead letter is terminal: re-delivery never re-executes it.
  const settled = await bridge.pollOnce();
  assert.equal(settled.outcomes.length, 0);
  assert.equal(calls.length, 1);
});

test('an expired award is dead-lettered on the first attempt with a reason', async () => {
  const { options, calls, data } = runtimeOptions();
  const expired = repoisoned(data, (payload) => {
    payload.expires_at = '2020-01-01T00:00:00.000Z';
  });
  const store = new MemoryBridgeStore();
  const bridge = new KernelMarketplaceBridge({
    ...options,
    sequencer: sequencerForAll([expired, followingAward(data)]),
    store,
  });

  const result = await bridge.pollOnce();
  assert.equal(result.outcomes[0].status, 'dead_lettered');
  assert.equal(result.outcomes[0].reason, 'award_expired');
  assert.equal(result.outcomes[1].status, 'completed');
  assert.equal(store.record(options.id, EVENT).attempts, 1, 'no pointless retries');
  assert.equal(calls.length, 1);
});

test('a binding verdict that cannot change is dead-lettered on the first attempt', async () => {
  // The binding has already looked at the input or the record's state and said
  // no. Retrying VALIDATION / PRECONDITION_FAILED / NOT_FOUND `maxAttempts`
  // times cannot change the answer, and on a bridge it holds the sequencer
  // cursor behind a message that can never land.
  for (const code of ['VALIDATION', 'PRECONDITION_FAILED', 'NOT_FOUND']) {
    const { options, data } = runtimeOptions();
    const store = new MemoryBridgeStore();
    const bridge = new KernelMarketplaceBridge({
      ...options,
      sequencer: sequencerForAll([data.sequenced]),
      store,
      maxAttempts: 5,
      commerce: {
        async executeKernelCommand() {
          const error = new Error(`refused: ${code}`);
          error.code = code;
          throw error;
        },
      },
    });

    const result = await bridge.pollOnce();
    assert.equal(result.outcomes[0].status, 'dead_lettered', code);
    assert.equal(result.outcomes[0].reason, code, 'the binding code is the dead-letter reason');
    assert.equal(
      store.record(options.id, EVENT).attempts,
      1,
      `${code}: a permanent verdict must not be retried`,
    );
    // ...and the cursor is past it, so later messages are not held hostage.
    assert.equal(result.nextSequence, 2, code);
    assert.equal(store.getCursor(options.id), 2, code);
  }
});

test('a transient binding failure is still retried to the attempt ceiling', async () => {
  const { options, data } = runtimeOptions();
  const store = new MemoryBridgeStore();
  const bridge = new KernelMarketplaceBridge({
    ...options,
    sequencer: sequencerForAll([data.sequenced]),
    store,
    maxAttempts: 3,
    commerce: {
      async executeKernelCommand() {
        const error = new Error('the database went away');
        error.code = 'DATABASE';
        throw error;
      },
    },
  });

  await assert.rejects(bridge.pollOnce(), /database went away/);
  await assert.rejects(bridge.pollOnce(), /database went away/);
  assert.equal(store.getCursor(options.id), 1, 'cursor holds while retries remain');
  const drained = await bridge.pollOnce();
  assert.equal(drained.outcomes[0].status, 'dead_lettered');
  assert.equal(drained.outcomes[0].reason, 'attempts_exhausted');
  assert.equal(store.record(options.id, EVENT).attempts, 3);
});

test('dead letters survive worker reconstruction in the durable store', async (t) => {
  let Database;
  try {
    ({ default: Database } = await import('better-sqlite3'));
  } catch {
    t.skip('better-sqlite3 is optional');
    return;
  }
  const db = new Database(':memory:');
  t.after(() => db.close());
  // A store created before dead-lettering existed rejects the new status.
  db.exec(`
    CREATE TABLE _stateset_marketplace_bridge_inbox (
      bridge_id TEXT NOT NULL, event_id TEXT NOT NULL, sequence_number INTEGER NOT NULL,
      event_digest TEXT NOT NULL,
      status TEXT NOT NULL CHECK (status IN ('processing', 'failed', 'completed', 'rejected')),
      attempts INTEGER NOT NULL DEFAULT 0, result_json TEXT, last_error TEXT,
      updated_at TEXT NOT NULL, PRIMARY KEY (bridge_id, event_id)
    );
    INSERT INTO _stateset_marketplace_bridge_inbox
      VALUES ('legacy-bridge', 'legacy-event', 1, 'digest', 'completed', 1, NULL, NULL, 'then');
  `);

  const { options, calls, data } = runtimeOptions();
  const expired = repoisoned(data, (payload) => {
    payload.expires_at = '2020-01-01T00:00:00.000Z';
  });
  const store = new SqliteBridgeStore(db);
  // The pre-existing row survives the schema upgrade.
  assert.equal(store.record('legacy-bridge', 'legacy-event').status, 'completed');

  const bridge = new KernelMarketplaceBridge({
    ...options,
    sequencer: sequencerForAll([expired, followingAward(data)]),
    store,
  });
  const result = await bridge.pollOnce();
  assert.equal(result.outcomes[0].status, 'dead_lettered');
  assert.equal(result.nextSequence, 3);
  assert.equal(calls.length, 1);

  const reopened = new SqliteBridgeStore(db);
  const record = reopened.record(options.id, EVENT);
  assert.equal(record.status, 'dead_lettered');
  assert.match(record.error, /unexpired deadline/);
  const again = await new KernelMarketplaceBridge({
    ...options,
    sequencer: sequencerForAll([expired, followingAward(data)]),
    store: reopened,
  }).pollOnce();
  assert.equal(again.outcomes.length, 0);
});
