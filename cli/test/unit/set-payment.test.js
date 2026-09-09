import test from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import {
  createAgentCommerce,
  SqlitePurchaseStore,
} from '../../../bindings/node/purchase-runtime.mjs';
import {
  createSetPaymentAdapter,
  SET_PAYMENT_SETTLED_TOPIC,
} from '../../../bindings/node/set-payment.mjs';
const addr = (n) => `0x${n.repeat(40)}`;
const hash = (n) => `0x${n.repeat(64)}`;
const word = (value) => value.slice(2).padStart(64, '0');

const sequencerCapabilities = async () => ({
  features: ['x402.client_intent_id.v1'],
  intent_id_encoding: 'uuid-prefix-zero-pad-bytes32',
});

test('sequencer profile uses a stable UUID v8 prefix with Rust-compatible padding', async () => {
  const f = fixture();
  const adapter = createSetPaymentAdapter({
    ...f.options,
    intentEncoding: 'sequencer-uuid-v1',
    getSequencerCapabilities: sequencerCapabilities,
  });
  const first = await adapter.execute(f.context);
  assert.equal(first.status, 'succeeded');
  const id = first.evidence.intent_id;
  assert.match(id, /^0x[0-9a-f]{12}8[0-9a-f]{3}[89ab][0-9a-f]{15}0{32}$/);
  const { sequencerUuidToBytes32 } = await import('../../src/x402/set-transaction.js');
  const hex = id.slice(2, 34);
  const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  assert.equal(sequencerUuidToBytes32(uuid), id);
  assert.deepEqual(await adapter.lookup(f.context), first);
  const legacy = fixture();
  assert.notEqual((await legacy.adapter.execute(legacy.context)).evidence.intent_id, id);
});

test('sequencer submission fails closed without matching capabilities; lookup remains available', async () => {
  for (const capabilities of [
    null,
    {},
    { features: ['x402.client_intent_id.v1'] },
    { features: [], intent_id_encoding: 'uuid-prefix-zero-pad-bytes32' },
  ]) {
    const f = fixture();
    const adapter = createSetPaymentAdapter({
      ...f.options,
      intentEncoding: 'sequencer-uuid-v1',
      getSequencerCapabilities: async () => capabilities,
    });
    await assert.rejects(adapter.execute(f.context), /does not support/);
    assert.equal(f.calls(), 0);
    assert.deepEqual(await adapter.lookup(f.context), { status: 'unknown' });
  }
  const f = fixture();
  assert.throws(
    () => createSetPaymentAdapter({ ...f.options, intentEncoding: 'sequencer-uuid-v1' }),
    /capability lookup/,
  );
  assert.throws(
    () => createSetPaymentAdapter({ ...f.options, intentEncoding: 'automatic' }),
    /unsupported/,
  );
});

test('purchase holds its budget until the Set adapter verifies the individual payment', async () => {
  const f = fixture();
  const db = new Database(':memory:');
  try {
    const store = new SqlitePurchaseStore(db);
    let orders = 0;
    const inventory = {
      execute: async () => ({ status: 'succeeded', evidence: { reservation_id: 'r1' } }),
      lookup: async () => ({ status: 'succeeded', evidence: { reservation_id: 'r1' } }),
    };
    const order = {
      execute: async () => {
        orders++;
        return { status: 'succeeded', evidence: { order_id: 'o1' } };
      },
      lookup: async () => ({ status: 'succeeded', evidence: { order_id: 'o1' } }),
    };
    const commerce = createAgentCommerce({
      store,
      identity: { agentId: 'buyer', principalId: 'company', tenantId: 'tenant', storeId: 'store' },
      policyVersion: 'v1',
      currencies: {
        USDC: {
          asset: f.context.operation.quote.asset,
          decimals: 6,
          payer: addr('2'),
          budgetId: 'budget',
        },
      },
      resolveQuote: async () => ({
        ...f.context.operation.quote,
        id: 'q1',
        counterpartyId: 'merchant',
      }),
      authorize: async () => ({ allowed: true }),
      allowApply: true,
      adapters: {
        reserve_inventory: inventory,
        confirm_inventory: inventory,
        release_inventory: inventory,
        create_order: order,
        pay: f.adapter,
      },
    });
    store.provisionBudget(commerce.scope, {
      id: 'budget',
      asset: f.context.operation.quote.asset,
      limit: '100',
      expiresAt: '2099-01-01T00:00:00Z',
    });
    f.state.missing = true;
    const pending = await commerce.buy({
      quoteId: 'q1',
      idempotencyKey: 'buy1',
      maxTotal: { amount: '50', currency: 'USDC' },
    });
    assert.equal(pending.status, 'reconciling');
    assert.equal(store.budget(commerce.scope, 'budget').reserved, '40');
    assert.equal(orders, 0);
    f.state.missing = false;
    f.state.mutate = (receipt) => {
      receipt.logs = [];
    };
    assert.equal((await commerce.resume(pending.id)).status, 'reconciling');
    assert.equal(store.budget(commerce.scope, 'budget').reserved, '40');
    assert.equal(orders, 0);
    f.state.mutate = () => {};
    const result = await commerce.resume(pending.id);
    assert.equal(result.status, 'completed');
    assert.equal(store.budget(commerce.scope, 'budget').spent, '40');
    assert.equal(store.budget(commerce.scope, 'budget').reserved, '0');
    assert.equal(f.calls(), 1);
    assert.equal(orders, 1);
    assert.equal(result.receipt.evidence.pay.finality, 'rpc_finalized');
  } finally {
    db.close();
  }
});
function fixture() {
  let submitted;
  let calls = 0;
  let lost = false;
  const tx = hash('a');
  const asset = `eip155:31337/erc20:${addr('1')}`;
  const context = {
    idempotencyKey: 'purchase:one:pay',
    operation: {
      quote: {
        amount: '40.000000',
        asset,
        payee: addr('3'),
        expiresAt: '2099-01-01T00:00:00Z',
      },
    },
  };
  const state = {
    chain: '0x7a69',
    finalized: { number: '0x20', hash: hash('c') },
    canonical: { number: '0x10', hash: hash('b') },
    mutate: () => {},
    missing: false,
  };
  const options = {
    chainId: '31337',
    settlementContract: addr('4'),
    token: addr('1'),
    payer: addr('2'),
    allowSubmit: true,
    submit: async (intent) => {
      calls++;
      submitted = intent;
      if (lost) throw new Error('response lost after broadcast');
      return tx;
    },
    findTransaction: async (intent) => {
      if (submitted) assert.deepEqual(intent, submitted);
      return submitted ? tx : null;
    },
    rpc: async (method) => {
      if (method === 'eth_chainId') return state.chain;
      if (method === 'eth_getBlockByNumber') throw new Error('use parameter-aware RPC');
      if (method !== 'eth_getTransactionReceipt') throw new Error('unexpected RPC');
      if (state.missing) return null;
      const receipt = {
        transactionHash: tx,
        blockHash: hash('b'),
        blockNumber: '0x10',
        status: '0x1',
        logs: [
          {
            address: addr('4'),
            topics: [
              SET_PAYMENT_SETTLED_TOPIC,
              hash('d'),
              submitted.intentId,
              `0x${word(addr('2'))}`,
            ],
            data: `0x${word(addr('3'))}${40000000n.toString(16).padStart(64, '0')}${word(addr('1'))}`,
            removed: false,
            transactionHash: tx,
            blockHash: hash('b'),
            blockNumber: '0x10',
            logIndex: '0x0',
          },
        ],
      };
      state.mutate(receipt);
      return receipt;
    },
  };
  const rpc = options.rpc;
  options.rpc = async (method, params) =>
    method === 'eth_getBlockByNumber'
      ? structuredClone(params[0] === 'finalized' ? state.finalized : state.canonical)
      : rpc(method, params);
  return {
    options,
    context,
    state,
    adapter: createSetPaymentAdapter(options),
    calls: () => calls,
    loseResponse: () => {
      lost = true;
    },
  };
}

test('exact finalized individual settlement produces purchase evidence', async () => {
  const f = fixture();
  const result = await f.adapter.execute(f.context);
  assert.equal(result.status, 'succeeded');
  assert.equal(result.evidence.amount, '40.000000');
  assert.equal(result.evidence.asset, f.context.operation.quote.asset);
  assert.equal(result.evidence.payee, addr('3'));
  assert.equal(result.evidence.finality, 'rpc_finalized');
  assert.equal(result.evidence.block_number, '16');
  assert.deepEqual(await f.adapter.lookup(f.context), result);
  assert.equal(f.calls(), 1);
});

test('lost submission response recovers through a newly constructed adapter without submission', async () => {
  const f = fixture();
  f.loseResponse();
  await assert.rejects(f.adapter.execute(f.context), /response lost/);
  const recovery = createSetPaymentAdapter({ ...f.options, allowSubmit: false });
  assert.equal((await recovery.lookup(f.context)).status, 'succeeded');
  assert.equal(f.calls(), 1);
});

for (const [name, mutate] of Object.entries({
  'successful batch without individual event': (r) => {
    r.logs = [];
  },
  'reverted transaction': (r) => {
    r.status = '0x0';
  },
  'wrong emitting contract': (r) => {
    r.logs[0].address = addr('5');
  },
  'different intent': (r) => {
    r.logs[0].topics[2] = hash('f');
  },
  'duplicate settlement event': (r) => {
    r.logs.push(structuredClone(r.logs[0]));
  },
})) {
  test(`${name} never authorizes completion or resubmission`, async () => {
    const f = fixture();
    f.state.mutate = mutate;
    assert.equal((await f.adapter.execute(f.context)).status, 'unknown');
    assert.equal((await f.adapter.lookup(f.context)).status, 'unknown');
    assert.equal(f.calls(), 1);
  });
}
for (const [name, mutate] of Object.entries({
  payer: (r) => {
    r.logs[0].topics[3] = `0x${word(addr('5'))}`;
  },
  payee: (r) => {
    r.logs[0].data = `0x${word(addr('5'))}${r.logs[0].data.slice(66)}`;
  },
  token: (r) => {
    r.logs[0].data = `${r.logs[0].data.slice(0, 130)}${word(addr('5'))}`;
  },
  amount: (r) => {
    r.logs[0].data = `0x${word(addr('3'))}${'0'.repeat(64)}${word(addr('1'))}`;
  },
  'removed log': (r) => {
    r.logs[0].removed = true;
  },
  'receipt transaction': (r) => {
    r.transactionHash = hash('f');
  },
  'log block': (r) => {
    r.logs[0].blockHash = hash('f');
  },
  'log transaction': (r) => {
    r.logs[0].transactionHash = hash('f');
  },
  'noncanonical address padding': (r) => {
    r.logs[0].topics[3] = `0x1${word(addr('2')).slice(1)}`;
  },
})) {
  test(`rejects mismatched ${name}`, async () => {
    const f = fixture();
    f.state.mutate = mutate;
    await assert.rejects(f.adapter.execute(f.context));
  });
}

test('pending receipts, finality delay, and reorgs remain unresolved', async () => {
  const f = fixture();
  f.state.missing = true;
  assert.equal((await f.adapter.execute(f.context)).status, 'pending');
  f.state.missing = false;
  f.state.finalized.number = '0xf';
  assert.equal((await f.adapter.lookup(f.context)).status, 'pending');
  f.state.finalized.number = '0x20';
  f.state.canonical.hash = hash('f');
  assert.equal((await f.adapter.lookup(f.context)).status, 'unknown');
  f.state.canonical.hash = hash('b');
  assert.equal((await f.adapter.lookup(f.context)).status, 'succeeded');
  assert.equal(f.calls(), 1);
});

test('unknown transaction and RPC failure never return not_found', async () => {
  const f = fixture();
  assert.equal((await f.adapter.lookup(f.context)).status, 'unknown');
  const adapter = createSetPaymentAdapter({
    ...f.options,
    rpc: async () => {
      throw new Error('RPC unavailable');
    },
  });
  await assert.rejects(adapter.lookup(f.context), /RPC unavailable/);
  assert.equal(f.calls(), 0);
});

test('submission requires opt-in, exact asset, precision, valid expiry and matching chain', async () => {
  const f = fixture();
  await assert.rejects(
    createSetPaymentAdapter({ ...f.options, allowSubmit: false }).execute(f.context),
    /disabled/,
  );
  for (const changes of [
    { amount: '40.0000001' },
    { amount: 40 },
    { asset: 'USDC' },
    { expiresAt: '2000-01-01T00:00:00Z' },
    { payee: addr('0') },
  ]) {
    const context = structuredClone(f.context);
    Object.assign(context.operation.quote, changes);
    await assert.rejects(f.adapter.execute(context));
  }
  f.state.chain = '0x1';
  await assert.rejects(f.adapter.execute(f.context), /chain mismatch/);
  assert.equal(f.calls(), 0);
});
