import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cliSrc = path.join(__dirname, '..', '..', 'src');

const { A2AStore } = await import(path.join(cliSrc, 'a2a', 'store.js'));
const { createAgentRuntime, makeCommerceProxy } = await import(
  path.join(cliSrc, 'a2a', 'agent-runtime.js')
);
const { createA2AService } = await import(path.join(cliSrc, 'a2a', 'index.js'));
const { a2aTools } = await import(path.join(cliSrc, 'tools', 'a2a.js'));
const { a2aObservabilityTools } = await import(path.join(cliSrc, 'tools', 'a2a-observability.js'));
const { createFinalityTracker } = await import(path.join(cliSrc, 'a2a', 'settlement-finality.js'));
const { _getRuntimeRegistry } = await import(path.join(cliSrc, 'tools', 'agent-runtime.js'));
const ORIGINAL_FETCH = global.fetch;

function findTool(name) {
  const tool = a2aTools.find((candidate) => candidate.name === name);
  if (!tool) {
    throw new Error(`Tool "${name}" not found`);
  }
  return tool;
}

function findObservabilityTool(name) {
  const tool = a2aObservabilityTools.find((candidate) => candidate.name === name);
  if (!tool) {
    throw new Error(`Observability tool "${name}" not found`);
  }
  return tool;
}

function wallet() {
  return '0x' + crypto.randomBytes(20).toString('hex');
}

function keys() {
  return {
    privateKey: crypto.randomBytes(32).toString('hex'),
    publicKey: crypto.randomBytes(32).toString('hex'),
  };
}

function createMockSettlement(overrides = {}) {
  const calls = { settle: [], hasSufficientFunds: [] };

  return {
    calls,
    service: {
      get chainId() {
        return overrides.chainId || 'bitcoin';
      },
      get isSimulation() {
        return overrides.simulated ?? true;
      },
      get agentId() {
        return overrides.agentId || 'tool-runtime';
      },
      async settle(params) {
        calls.settle.push(params);
        return {
          success: true,
          txHash: overrides.txHash || '0x' + 'd'.repeat(64),
          blockNumber: overrides.blockNumber || 99,
          explorerUrl: overrides.explorerUrl || 'https://example.test/tx/tool-runtime',
          confirmations: overrides.confirmations ?? 1,
          simulated: overrides.simulated ?? true,
        };
      },
      async hasSufficientFunds(amount) {
        calls.hasSufficientFunds.push(amount);
        return {
          sufficient: true,
          balance: '100.0',
          required: String(amount),
          symbol: overrides.symbol || 'BTC',
        };
      },
      async getBalance() {
        return {
          balance: '100.0',
          balanceSmallest: 100000000n,
          symbol: overrides.symbol || 'BTC',
        };
      },
      async getAddress() {
        return overrides.address || 'bc1qtoolruntimewallet';
      },
    },
  };
}

let dbPath;
let store;
let commerce;

beforeEach(() => {
  dbPath = path.join(
    __dirname,
    `.test-a2a-runtime-routing-${Date.now()}-${Math.random().toString(36).slice(2)}.db`,
  );
  store = new A2AStore({ dbPath });
  store.init();
  commerce = makeCommerceProxy(store);
});

afterEach(() => {
  const registry = _getRuntimeRegistry();
  for (const runtime of registry.values()) {
    try {
      runtime.destroy?.();
    } catch {
      // Best effort cleanup.
    }
  }
  registry.clear();

  try {
    store.close();
  } catch {
    // Ignore close failures during cleanup.
  }
  try {
    fs.unlinkSync(dbPath);
  } catch {
    // Ignore missing temp files.
  }
  global.fetch = ORIGINAL_FETCH;
});

async function invoke(toolName, params = {}, opts = {}) {
  const tool = findTool(toolName);
  return tool.handler({
    commerce,
    params,
    allowApply: opts.allowApply ?? true,
    agentConfig: opts.agentConfig ?? {},
  });
}

describe('A2A tools reuse active runtimes for native settlement', () => {
  it('a2a_request_payment uses the runtime receive address for shielded ZEC requests', async () => {
    const runtime = createAgentRuntime({
      name: 'RequestToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'zcash',
      symbol: 'ZEC',
      address: 'u1requesttoolwallet',
    });
    runtime.settlement = settlement.service;

    const result = await invoke(
      'a2a_request_payment',
      {
        amount: 0.75,
        description: 'Shielded runtime invoice',
        asset: 'ZEC',
        network: 'zcash',
      },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['zcash']);
    assert.equal(result.request.asset, 'ZEC');
    assert.equal(result.request.network, 'zcash');
    assert.equal(result.request.requesterPaymentAddress, 'u1requesttoolwallet');

    const stored = store.getPaymentRequest(result.request.id);
    assert.equal(JSON.parse(stored.metadata).requester_payment_address, 'u1requesttoolwallet');
  });

  it('a2a_request_quote uses runtime payment defaults when asset and network are omitted', async () => {
    const runtime = createAgentRuntime({
      name: 'QuoteToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'bitcoin',
      symbol: 'BTC',
      address: 'bc1qquotetoolwallet',
    });
    runtime.settlement = settlement.service;

    const sellerIdentity = wallet();
    commerce.x402().registerAgent({
      id: crypto.randomUUID(),
      name: 'Quoted Seller',
      wallet_address: sellerIdentity,
      public_key: keys().publicKey,
      supported_networks: ['bitcoin'],
      supported_assets: ['BTC'],
      payment_addresses: { bitcoin: 'bc1qquotedtoolpayout' },
      trust_level: 'sandbox',
    });

    const result = await invoke(
      'a2a_request_quote',
      {
        seller: sellerIdentity,
        items: [{ description: 'Runtime default quote' }],
      },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['bitcoin']);
    assert.equal(result.quote.asset, 'BTC');
    assert.equal(result.quote.network, 'bitcoin');

    const storedQuote = store.getQuote(result.quote.id);
    assert.equal(storedQuote.asset, 'BTC');
    assert.deepEqual(storedQuote.accepted_networks, ['bitcoin']);
    assert.equal(JSON.parse(storedQuote.metadata).seller_payment_address, 'bc1qquotedtoolpayout');
  });

  it('a2a_pay uses the runtime settlement path for BTC direct payments', async () => {
    const runtime = createAgentRuntime({
      name: 'DirectPayToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'bitcoin',
      symbol: 'BTC',
      address: 'bc1qdirecttoolwallet',
    });
    runtime.settlement = settlement.service;

    const sellerIdentity = wallet();
    commerce.x402().registerAgent({
      id: crypto.randomUUID(),
      name: 'BTC Seller',
      wallet_address: sellerIdentity,
      public_key: keys().publicKey,
      supported_networks: ['bitcoin'],
      supported_assets: ['BTC'],
      payment_addresses: { bitcoin: 'bc1qdirectsellerpayout' },
      trust_level: 'sandbox',
    });

    const result = await invoke(
      'a2a_pay',
      {
        to: sellerIdentity,
        amount: 0.0021,
        asset: 'BTC',
        network: 'bitcoin',
        memo: 'Direct runtime BTC payment',
      },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['bitcoin']);
    assert.equal(result.payment.status, 'completed');
    assert.equal(result.payment.to, 'bc1qdirectsellerpayout');
    assert.equal(settlement.calls.hasSufficientFunds[0], 0.0021);
    assert.equal(settlement.calls.settle[0].toAddress, 'bc1qdirectsellerpayout');

    const payments = store.listPayments({ sender_address: runtime.walletAddress });
    assert.equal(payments.length, 1);
    assert.equal(payments[0].status, 'completed');
    assert.equal(payments[0].recipient_address, 'bc1qdirectsellerpayout');
    assert.equal(payments[0].network, 'bitcoin');
  });

  it('a2a_pay updates the finality tracker for runtime-settled bitcoin payments', async () => {
    commerce._finalityTracker = createFinalityTracker();

    const runtime = createAgentRuntime({
      name: 'TrackedDirectPayRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'bitcoin',
      symbol: 'BTC',
      address: 'bc1qtrackedtoolwallet',
      simulated: false,
      confirmations: 6,
      blockNumber: 144,
      txHash: '0x' + 'f'.repeat(64),
      explorerUrl: 'https://example.test/tx/runtime-finality',
    });
    runtime.settlement = settlement.service;

    const sellerIdentity = wallet();
    commerce.x402().registerAgent({
      id: crypto.randomUUID(),
      name: 'Tracked BTC Seller',
      wallet_address: sellerIdentity,
      public_key: keys().publicKey,
      supported_networks: ['bitcoin'],
      supported_assets: ['BTC'],
      payment_addresses: { bitcoin: 'bc1qtrackedsellerpayout' },
      trust_level: 'sandbox',
    });

    const result = await invoke(
      'a2a_pay',
      {
        to: sellerIdentity,
        amount: 0.001,
        asset: 'BTC',
        network: 'bitcoin',
        memo: 'Tracked runtime BTC payment',
      },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.payment.status, 'completed');

    const settlementStatus = await findObservabilityTool('a2a_settlement_status').handler({
      commerce,
      params: { intentId: result.payment.id },
    });
    assert.equal(settlementStatus.state, 'final');
    assert.equal(settlementStatus.confirmations, 6);
    assert.equal(settlementStatus.chain, 'bitcoin');
  });

  it('a2a_pay_request uses the runtime settlement path for ZEC requests', async () => {
    const runtime = createAgentRuntime({
      name: 'PayRequestToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'zcash',
      symbol: 'ZEC',
      address: 'u1payrequesttoolwallet',
    });
    runtime.settlement = settlement.service;

    const requesterIdentity = wallet();
    const requesterPayout = 'u1payrequestrecipient';
    commerce.x402().registerAgent({
      id: crypto.randomUUID(),
      name: 'ZEC Requester',
      wallet_address: requesterIdentity,
      public_key: keys().publicKey,
      supported_networks: ['zcash'],
      supported_assets: ['ZEC'],
      payment_addresses: { zcash: requesterPayout },
      trust_level: 'sandbox',
    });

    const requesterSvc = createA2AService(commerce, {
      agentId: crypto.randomUUID(),
      walletAddress: requesterIdentity,
      defaultAsset: 'ZEC',
      defaultNetwork: 'zcash',
      receiveAddressForNetwork: async () => requesterPayout,
    });
    const request = await requesterSvc.requestPayment({
      amount: 0.5,
      description: 'Shielded request',
      asset: 'ZEC',
      network: 'zcash',
      allowPartial: true,
    });

    const result = await invoke(
      'a2a_pay_request',
      { requestId: request.request.id },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['zcash']);
    assert.equal(result.request.status, 'paid');
    assert.equal(result.payment.status, 'completed');
    assert.equal(result.payment.to, requesterPayout);
    assert.equal(settlement.calls.hasSufficientFunds[0], 0.5);
    assert.equal(settlement.calls.settle[0].toAddress, requesterPayout);

    const storedRequest = store.getPaymentRequest(request.request.id);
    assert.equal(storedRequest.status, 'paid');
    assert.equal(storedRequest.amount_paid, storedRequest.amount);

    const payments = store.listPayments({ sender_address: runtime.walletAddress });
    assert.equal(payments.length, 1);
    assert.equal(payments[0].status, 'completed');
    assert.equal(payments[0].recipient_address, requesterPayout);
    assert.equal(payments[0].network, 'zcash');
  });

  it('a2a_accept_quote uses the runtime settlement path for BTC quotes', async () => {
    const runtime = createAgentRuntime({
      name: 'AcceptQuoteToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'bitcoin',
      symbol: 'BTC',
      address: 'bc1qaccepttoolwallet',
    });
    runtime.settlement = settlement.service;

    const sellerIdentity = wallet();
    commerce.x402().registerAgent({
      id: crypto.randomUUID(),
      name: 'BTC Quote Seller',
      wallet_address: sellerIdentity,
      public_key: keys().publicKey,
      supported_networks: ['bitcoin'],
      supported_assets: ['BTC'],
      payment_addresses: { bitcoin: 'bc1qquotesellerpayout' },
      trust_level: 'sandbox',
    });

    const requested = await invoke(
      'a2a_request_quote',
      {
        seller: sellerIdentity,
        items: [{ description: 'Runtime-priced service' }],
        asset: 'BTC',
        network: 'bitcoin',
      },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );
    await invoke(
      'a2a_provide_quote',
      {
        quoteId: requested.quote.id,
        total: 0.015,
      },
      { agentConfig: { walletAddress: sellerIdentity } },
    );

    const result = await invoke(
      'a2a_accept_quote',
      { quoteId: requested.quote.id },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['bitcoin']);
    assert.equal(result.quote.status, 'accepted');
    assert.equal(result.quote.network, 'bitcoin');
    assert.equal(result.payment.status, 'completed');
    assert.equal(result.payment.to, 'bc1qquotesellerpayout');
    assert.equal(settlement.calls.hasSufficientFunds[0], 0.015);
    assert.equal(settlement.calls.settle[0].toAddress, 'bc1qquotesellerpayout');

    const storedQuote = store.getQuote(requested.quote.id);
    assert.equal(storedQuote.status, 'accepted');
    assert.ok(storedQuote.payment_id);

    const payments = store.listPayments({ sender_address: runtime.walletAddress });
    assert.equal(payments.length, 1);
    assert.equal(payments[0].status, 'completed');
    assert.equal(payments[0].recipient_address, 'bc1qquotesellerpayout');
    assert.equal(payments[0].network, 'bitcoin');
  });

  it('a2a_process_subscription_billing uses the runtime settlement path for BTC billing', async () => {
    const runtime = createAgentRuntime({
      name: 'BillingToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'bitcoin',
      symbol: 'BTC',
      address: 'bc1qtoolbuyerwallet',
    });
    runtime.settlement = settlement.service;

    const providerIdentity = wallet();
    commerce.x402().registerAgent({
      id: crypto.randomUUID(),
      name: 'BTC Provider',
      wallet_address: providerIdentity,
      public_key: keys().publicKey,
      supported_networks: ['bitcoin'],
      supported_assets: ['BTC'],
      payment_addresses: { bitcoin: 'bc1qtoolproviderpayout' },
      trust_level: 'sandbox',
    });

    const overdueAt = new Date(Date.now() - 60_000).toISOString();
    store.createSubscription({
      id: 'sub-tool-owned',
      subscriber_address: runtime.walletAddress,
      provider_address: providerIdentity,
      plan_name: 'BTC Tool Feed',
      status: 'active',
      amount: 150000,
      amount_decimal: 0.0015,
      asset: 'BTC',
      network: 'bitcoin',
      billing_interval: 'monthly',
      current_period_start: overdueAt,
      current_period_end: overdueAt,
      next_billing_date: overdueAt,
      cancel_at_period_end: false,
      past_due_since: null,
      max_past_due_cycles: 3,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
    });

    const result = await invoke(
      'a2a_process_subscription_billing',
      {},
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['bitcoin']);
    assert.equal(result.billing.billingCount, 1);
    assert.equal(settlement.calls.hasSufficientFunds[0], 0.0015);
    assert.equal(settlement.calls.settle[0].toAddress, 'bc1qtoolproviderpayout');

    const sub = store.getSubscription('sub-tool-owned');
    assert.equal(sub.billing_count, 1);
    assert.ok(sub.last_payment_id);

    const payments = store.listPayments({ sender_address: runtime.walletAddress });
    assert.equal(payments.length, 1);
    assert.equal(payments[0].status, 'completed');
    assert.equal(payments[0].recipient_address, 'bc1qtoolproviderpayout');
  });

  it('a2a_execute_split_payment uses the runtime settlement path for ZEC splits', async () => {
    const runtime = createAgentRuntime({
      name: 'SplitToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'zcash',
      symbol: 'ZEC',
      address: 'u1toolruntimewallet',
    });
    runtime.settlement = settlement.service;

    const created = await invoke('a2a_create_split_payment', {
      senderAddress: runtime.walletAddress,
      totalAmount: 0.5,
      asset: 'ZEC',
      network: 'zcash',
      recipients: [
        { address: 'u1alice', percent: 60 },
        { address: 'u1bob', percent: 40 },
      ],
      memo: 'Shielded tool split',
    });

    const result = await invoke(
      'a2a_execute_split_payment',
      { splitPaymentId: created.splitPayment.id },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['zcash']);
    assert.equal(settlement.calls.hasSufficientFunds.length, 2);
    assert.equal(settlement.calls.settle.length, 2);
    assert.equal(settlement.calls.settle[0].toAddress, 'u1alice');
    assert.equal(settlement.calls.settle[1].toAddress, 'u1bob');

    const payments = store.listPayments({ sender_address: runtime.walletAddress });
    assert.equal(payments.length, 2);
    assert.ok(payments.every((payment) => payment.network === 'zcash'));
    assert.ok(payments.every((payment) => payment.status === 'completed'));
  });

  it('a2a_get_payment can refresh native bitcoin settlement state', async () => {
    const agentWallet = wallet();
    const now = new Date().toISOString();
    const paymentId = crypto.randomUUID();
    const txHash = 'c'.repeat(64);
    commerce._finalityTracker = createFinalityTracker();

    store.createPayment({
      id: paymentId,
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qtoolrecipient',
      amount: 500000,
      amount_decimal: 0.005,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Inspectable payment',
      idempotency_key: `inspect-${paymentId}`,
      intent_id: null,
      tx_hash: txHash,
      block_number: null,
      metadata: JSON.stringify({ chain_id: 'bitcoin' }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${txHash}/status`)) {
        return {
          ok: true,
          async json() {
            return {
              confirmed: true,
              block_height: 200,
              block_time: 1_710_000_200,
            };
          },
          async text() {
            return JSON.stringify({
              confirmed: true,
              block_height: 200,
              block_time: 1_710_000_200,
            });
          },
        };
      }

      if (url.endsWith('/blocks/tip/height')) {
        return {
          ok: true,
          async json() {
            return 205;
          },
          async text() {
            return '205';
          },
        };
      }

      throw new Error(`Unexpected fetch: ${url}`);
    };

    const result = await invoke(
      'a2a_get_payment',
      {
        paymentId,
        refreshOnChain: true,
      },
      { agentConfig: { walletAddress: agentWallet } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, false);
    assert.equal(result.refreshed, true);
    assert.equal(result.payment.id, paymentId);
    assert.equal(result.payment.status, 'completed');
    assert.equal(result.payment.blockNumber, 200);
    assert.equal(result.payment.confirmations, 6);
    assert.equal(result.onChain.final, true);
    assert.equal(result.onChain.requiredConfirmations, 6);
    assert.equal(result.finality?.state, 'final');
    assert.equal(result.finality?.confirmations, 6);

    const settlementStatus = await findObservabilityTool('a2a_settlement_status').handler({
      commerce,
      params: { intentId: paymentId },
    });
    assert.equal(settlementStatus.state, 'final');
    assert.equal(settlementStatus.confirmations, 6);

    const stored = store.getPayment(paymentId);
    assert.equal(stored.status, 'completed');
    assert.equal(stored.block_number, 200);
    assert.equal(JSON.parse(stored.metadata).confirmations, 6);
  });

  it('a2a_list_payments can bulk-refresh submitted bitcoin payments', async () => {
    commerce._finalityTracker = createFinalityTracker();
    const agentWallet = wallet();
    const now = new Date().toISOString();

    store.createPayment({
      id: crypto.randomUUID(),
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qbulkone',
      amount: 400000,
      amount_decimal: 0.004,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Bulk one',
      idempotency_key: 'bulk-one',
      intent_id: null,
      tx_hash: '1'.repeat(64),
      block_number: null,
      metadata: JSON.stringify({ chain_id: 'bitcoin' }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qbulktwo',
      amount: 500000,
      amount_decimal: 0.005,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Bulk two',
      idempotency_key: 'bulk-two',
      intent_id: null,
      tx_hash: '2'.repeat(64),
      block_number: null,
      metadata: JSON.stringify({ chain_id: 'bitcoin' }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${'1'.repeat(64)}/status`)) {
        return {
          ok: true,
          async json() {
            return { confirmed: true, block_height: 210, block_time: 1_710_000_210 };
          },
          async text() {
            return JSON.stringify({ confirmed: true, block_height: 210, block_time: 1_710_000_210 });
          },
        };
      }
      if (url.endsWith(`/tx/${'2'.repeat(64)}/status`)) {
        return {
          ok: true,
          async json() {
            return { confirmed: false };
          },
          async text() {
            return JSON.stringify({ confirmed: false });
          },
        };
      }
      if (url.endsWith('/blocks/tip/height')) {
        return {
          ok: true,
          async json() {
            return 215;
          },
          async text() {
            return '215';
          },
        };
      }
      throw new Error(`Unexpected fetch: ${url}`);
    };

    const result = await invoke(
      'a2a_list_payments',
      {
        direction: 'sent',
        refreshOnChain: true,
      },
      { agentConfig: { walletAddress: agentWallet } },
    );

    assert.equal(result.success, true);
    assert.equal(result.refreshed, true);
    assert.equal(result.count, 2);
    const completed = result.payments.find((payment) => payment.txHash === '1'.repeat(64));
    const pending = result.payments.find((payment) => payment.txHash === '2'.repeat(64));
    assert.equal(completed.status, 'completed');
    assert.equal(completed.finality?.state, 'final');
    assert.equal(pending.status, 'submitted');
    assert.equal(pending.finality?.state, 'unconfirmed');
  });

  it('a2a_list_payments can filter by native settlement network', async () => {
    const agentWallet = wallet();
    const now = new Date().toISOString();

    store.createPayment({
      id: crypto.randomUUID(),
      status: 'completed',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qfilteredbitcoin',
      amount: 300000,
      amount_decimal: 0.003,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Filtered BTC payment',
      idempotency_key: 'filter-btc',
      intent_id: null,
      tx_hash: 'a'.repeat(64),
      block_number: 300,
      metadata: JSON.stringify({ chain_id: 'bitcoin', confirmations: 6 }),
      created_at: now,
      updated_at: now,
      completed_at: now,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'u1filteredzec',
      amount: 200000,
      amount_decimal: 0.002,
      asset: 'ZEC',
      network: 'zcash',
      memo: 'Filtered ZEC payment',
      idempotency_key: 'filter-zec',
      intent_id: null,
      tx_hash: 'b'.repeat(64),
      block_number: 301,
      metadata: JSON.stringify({ chain_id: 'zcash', confirmations: 3 }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    const result = await invoke(
      'a2a_list_payments',
      {
        direction: 'sent',
        network: 'bitcoin',
      },
      { agentConfig: { walletAddress: agentWallet } },
    );

    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.payments[0].network, 'bitcoin');
    assert.equal(result.payments[0].asset, 'BTC');
    assert.equal(result.payments[0].to, 'bc1qfilteredbitcoin');
  });

  it('a2a_get_balance returns a filtered native-rail breakdown through the runtime surface', async () => {
    const runtime = createAgentRuntime({
      name: 'BalanceToolRuntime',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      budget: { daily: 1000, perTransaction: 1000 },
      logger: () => {},
    });
    _getRuntimeRegistry().set(runtime.name, runtime);

    const settlement = createMockSettlement({
      chainId: 'bitcoin',
      symbol: 'BTC',
      address: 'bc1qbalancetoolwallet',
    });
    runtime.settlement = settlement.service;

    const now = new Date().toISOString();
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'completed',
      sender_agent_id: crypto.randomUUID(),
      sender_address: runtime.walletAddress,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qbalancesent',
      amount: 300000,
      amount_decimal: 0.003,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'BTC outgoing',
      idempotency_key: 'balance-btc-sent',
      intent_id: null,
      tx_hash: '1'.repeat(64),
      block_number: 500,
      metadata: JSON.stringify({ chain_id: 'bitcoin', confirmations: 6 }),
      created_at: now,
      updated_at: now,
      completed_at: now,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'completed',
      sender_agent_id: crypto.randomUUID(),
      sender_address: 'bc1qcounterparty',
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: runtime.walletAddress,
      amount: 100000,
      amount_decimal: 0.001,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'BTC incoming',
      idempotency_key: 'balance-btc-received',
      intent_id: null,
      tx_hash: '2'.repeat(64),
      block_number: 501,
      metadata: JSON.stringify({ chain_id: 'bitcoin', confirmations: 6 }),
      created_at: now,
      updated_at: now,
      completed_at: now,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'completed',
      sender_agent_id: crypto.randomUUID(),
      sender_address: runtime.walletAddress,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'u1balancezec',
      amount: 500000,
      amount_decimal: 0.005,
      asset: 'ZEC',
      network: 'zcash',
      memo: 'ZEC outgoing',
      idempotency_key: 'balance-zec-sent',
      intent_id: null,
      tx_hash: '3'.repeat(64),
      block_number: 600,
      metadata: JSON.stringify({ chain_id: 'zcash', confirmations: 10 }),
      created_at: now,
      updated_at: now,
      completed_at: now,
    });

    const result = await invoke(
      'a2a_get_balance',
      {
        network: 'bitcoin',
      },
      { agentConfig: { walletAddress: runtime.walletAddress } },
    );

    assert.equal(result.success, true);
    assert.equal(result.viaRuntime, true);
    assert.deepEqual(result.settlementChains, ['bitcoin']);
    assert.ok(Math.abs(result.balance.totalSent - 0.003) < 1e-12);
    assert.ok(Math.abs(result.balance.totalReceived - 0.001) < 1e-12);
    assert.ok(Math.abs(result.balance.netFlow + 0.002) < 1e-12);
    assert.equal(result.balance.aggregateTotalsMeaningful, true);
    assert.equal(result.balance.aggregateAsset, 'BTC');
    assert.deepEqual(result.balance.assets, ['BTC']);
    assert.equal(result.balance.paymentCountSent, 1);
    assert.equal(result.balance.paymentCountReceived, 1);
    assert.equal(result.balance.paymentCount, 2);
    assert.equal(result.balance.summarySource, 'store_aggregate');
    assert.ok(Math.abs(result.balance.breakdownByAsset.BTC.totalSent - 0.003) < 1e-12);
    assert.ok(Math.abs(result.balance.breakdownByAsset.BTC.totalReceived - 0.001) < 1e-12);
    assert.ok(Math.abs(result.balance.breakdownByAsset.BTC.networks.bitcoin.netFlow + 0.002) < 1e-12);
  });

  it('a2a_settlement_finality_metrics computes persisted metrics for filtered bitcoin payments', async () => {
    const agentWallet = wallet();
    const createdAt = '2026-01-01T00:00:00.000Z';
    const completedAt = '2026-01-01T00:05:00.000Z';
    const failedAt = '2026-01-01T00:06:00.000Z';

    store.createPayment({
      id: crypto.randomUUID(),
      status: 'completed',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qmetricfinal',
      amount: 450000,
      amount_decimal: 0.0045,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Final BTC payment',
      idempotency_key: 'metric-final-btc',
      intent_id: null,
      tx_hash: 'c'.repeat(64),
      block_number: 310,
      metadata: JSON.stringify({ chain_id: 'bitcoin', confirmations: 6 }),
      created_at: createdAt,
      updated_at: completedAt,
      completed_at: completedAt,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'failed',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qmetricfailed',
      amount: 125000,
      amount_decimal: 0.00125,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Failed BTC payment',
      idempotency_key: 'metric-failed-btc',
      intent_id: null,
      tx_hash: 'd'.repeat(64),
      block_number: 311,
      metadata: JSON.stringify({
        chain_id: 'bitcoin',
        settlement_error: 'broadcast_failed',
      }),
      created_at: createdAt,
      updated_at: failedAt,
      completed_at: null,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'u1metricpending',
      amount: 330000,
      amount_decimal: 0.0033,
      asset: 'ZEC',
      network: 'zcash',
      memo: 'Pending ZEC payment',
      idempotency_key: 'metric-pending-zec',
      intent_id: null,
      tx_hash: 'e'.repeat(64),
      block_number: 410,
      metadata: JSON.stringify({ chain_id: 'zcash', confirmations: 3 }),
      created_at: createdAt,
      updated_at: createdAt,
      completed_at: null,
    });

    const result = await findObservabilityTool('a2a_settlement_finality_metrics').handler({
      commerce,
      params: {
        agentAddress: agentWallet,
        network: 'bitcoin',
      },
    });

    assert.equal(result.totalTracked, 2);
    assert.equal(result.totalFinal, 1);
    assert.equal(result.totalFailed, 1);
    assert.equal(result.pendingCount, 0);
    assert.equal(result.avgConfirmationTimeMs, 300000);
    assert.equal(result.finalityRate, 0.5);
    assert.equal(result.totalReorgs, 0);
    assert.equal(result.historyHydrated, true);
    assert.equal(result.reorgCountSource, 'not_persisted');
    assert.equal(result.refreshed, false);
    assert.equal(result.filters.agentAddress, agentWallet);
    assert.equal(result.filters.network, 'bitcoin');
  });

  it('a2a_settlement_finality_metrics can refresh pending bitcoin settlements before computing metrics', async () => {
    const agentWallet = wallet();
    const now = new Date().toISOString();
    const paymentId = crypto.randomUUID();

    store.createPayment({
      id: paymentId,
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qmetricrefresh',
      amount: 275000,
      amount_decimal: 0.00275,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Refreshable BTC payment',
      idempotency_key: 'metric-refresh-btc',
      intent_id: null,
      tx_hash: 'f'.repeat(64),
      block_number: null,
      metadata: JSON.stringify({ chain_id: 'bitcoin' }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${'f'.repeat(64)}/status`)) {
        return {
          ok: true,
          async json() {
            return { confirmed: true, block_height: 420, block_time: 1_710_000_420 };
          },
          async text() {
            return JSON.stringify({ confirmed: true, block_height: 420, block_time: 1_710_000_420 });
          },
        };
      }
      if (url.endsWith('/blocks/tip/height')) {
        return {
          ok: true,
          async json() {
            return 425;
          },
          async text() {
            return '425';
          },
        };
      }
      throw new Error(`Unexpected fetch: ${url}`);
    };

    const result = await findObservabilityTool('a2a_settlement_finality_metrics').handler({
      commerce,
      params: {
        agentAddress: agentWallet,
        network: 'bitcoin',
        refreshOnChain: true,
      },
    });

    assert.equal(result.totalTracked, 1);
    assert.equal(result.totalFinal, 1);
    assert.equal(result.totalFailed, 0);
    assert.equal(result.pendingCount, 0);
    assert.equal(result.refreshed, true);

    const stored = store.getPayment(paymentId);
    assert.equal(stored.status, 'completed');
    assert.equal(stored.block_number, 420);
    assert.equal(JSON.parse(stored.metadata).confirmations, 6);
  });

  it('a2a_settlement_pending hydrates pending settlements from stored payments', async () => {
    const agentWallet = wallet();
    const now = new Date().toISOString();

    store.createPayment({
      id: crypto.randomUUID(),
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'u1pendingzec',
      amount: 150000,
      amount_decimal: 0.0015,
      asset: 'ZEC',
      network: 'zcash',
      memo: 'Pending shielded payment',
      idempotency_key: 'pending-zec',
      intent_id: null,
      tx_hash: '3'.repeat(64),
      block_number: 300,
      metadata: JSON.stringify({ chain_id: 'zcash', confirmations: 3 }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    delete commerce._finalityTracker;

    const pending = await findObservabilityTool('a2a_settlement_pending').handler({
      commerce,
      params: { agentAddress: agentWallet, limit: 20 },
    });

    assert.equal(Array.isArray(pending), true);
    assert.equal(pending.length, 1);
    assert.equal(pending[0].chain, 'zcash');
    assert.equal(pending[0].confirmations, 3);
    assert.equal(pending[0].state, 'confirming');
  });

  it('a2a_settlement_status can refresh a bitcoin settlement through the payment layer', async () => {
    const agentWallet = wallet();
    const now = new Date().toISOString();
    const paymentId = crypto.randomUUID();
    const txHash = '4'.repeat(64);

    store.createPayment({
      id: paymentId,
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qstatusrefresh',
      amount: 250000,
      amount_decimal: 0.0025,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Settlement status refresh',
      idempotency_key: 'status-refresh',
      intent_id: null,
      tx_hash: txHash,
      block_number: null,
      metadata: JSON.stringify({ chain_id: 'bitcoin' }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${txHash}/status`)) {
        return {
          ok: true,
          async json() {
            return { confirmed: true, block_height: 220, block_time: 1_710_000_220 };
          },
          async text() {
            return JSON.stringify({ confirmed: true, block_height: 220, block_time: 1_710_000_220 });
          },
        };
      }
      if (url.endsWith('/blocks/tip/height')) {
        return {
          ok: true,
          async json() {
            return 225;
          },
          async text() {
            return '225';
          },
        };
      }
      throw new Error(`Unexpected fetch: ${url}`);
    };

    const result = await findObservabilityTool('a2a_settlement_status').handler({
      commerce,
      params: {
        intentId: paymentId,
        agentAddress: agentWallet,
        refreshOnChain: true,
      },
    });

    assert.equal(result.state, 'final');
    assert.equal(result.confirmations, 6);
    assert.equal(result.refreshed, true);
    assert.equal(result.payment.status, 'completed');
    assert.equal(result.onChain.final, true);

    const stored = store.getPayment(paymentId);
    assert.equal(stored.status, 'completed');
    assert.equal(stored.block_number, 220);
  });

  it('a2a_settlement_pending can refresh bitcoin settlements and keep zcash pending', async () => {
    const agentWallet = wallet();
    const now = new Date().toISOString();
    const btcPaymentId = crypto.randomUUID();
    const btcTxHash = '5'.repeat(64);

    store.createPayment({
      id: btcPaymentId,
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'bc1qpendingbtc',
      amount: 350000,
      amount_decimal: 0.0035,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Pending BTC settlement',
      idempotency_key: 'pending-btc',
      intent_id: null,
      tx_hash: btcTxHash,
      block_number: null,
      metadata: JSON.stringify({ chain_id: 'bitcoin' }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });
    store.createPayment({
      id: crypto.randomUUID(),
      status: 'submitted',
      sender_agent_id: crypto.randomUUID(),
      sender_address: agentWallet,
      recipient_agent_id: crypto.randomUUID(),
      recipient_address: 'u1pendingzecrefresh',
      amount: 120000,
      amount_decimal: 0.0012,
      asset: 'ZEC',
      network: 'zcash',
      memo: 'Pending ZEC settlement',
      idempotency_key: 'pending-zec-refresh',
      intent_id: null,
      tx_hash: '6'.repeat(64),
      block_number: 410,
      metadata: JSON.stringify({ chain_id: 'zcash', confirmations: 3 }),
      created_at: now,
      updated_at: now,
      completed_at: null,
    });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${btcTxHash}/status`)) {
        return {
          ok: true,
          async json() {
            return { confirmed: true, block_height: 230, block_time: 1_710_000_230 };
          },
          async text() {
            return JSON.stringify({ confirmed: true, block_height: 230, block_time: 1_710_000_230 });
          },
        };
      }
      if (url.endsWith('/blocks/tip/height')) {
        return {
          ok: true,
          async json() {
            return 235;
          },
          async text() {
            return '235';
          },
        };
      }
      throw new Error(`Unexpected fetch: ${url}`);
    };

    const pending = await findObservabilityTool('a2a_settlement_pending').handler({
      commerce,
      params: {
        agentAddress: agentWallet,
        refreshOnChain: true,
        limit: 20,
      },
    });

    assert.equal(Array.isArray(pending), true);
    assert.equal(pending.length, 1);
    assert.equal(pending[0].chain, 'zcash');
    assert.equal(pending[0].state, 'confirming');
    assert.equal(store.getPayment(btcPaymentId).status, 'completed');
  });
});
