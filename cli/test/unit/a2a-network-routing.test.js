import { afterEach, beforeEach, describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createA2AService } from '../../src/a2a/index.js';
import { createFinalityTracker } from '../../src/a2a/settlement-finality.js';

const BUYER = '0xBuyerAgent00000000000000000000000000000000';
const SELLER = '0xSellerAgent000000000000000000000000000000';
const SELLER_BITCOIN = 'bc1qselleragent';
const SELLER_ZCASH = 'u1selleragent';
const ORIGINAL_FETCH = global.fetch;
const SELLER_AGENT = {
  id: '11111111-1111-1111-1111-111111111111',
  walletAddress: SELLER,
  paymentAddress: SELLER_BITCOIN,
};

function createMockCommerce() {
  const paymentRequests = new Map();
  const quotes = new Map();
  const payments = [];
  const agentsById = new Map([
    [
      SELLER_AGENT.id,
      {
        id: SELLER_AGENT.id,
        wallet_address: SELLER,
        payment_addresses: JSON.stringify({
          bitcoin: SELLER_BITCOIN,
          zcash: SELLER_ZCASH,
        }),
      },
    ],
  ]);
  const agentsByWallet = new Map([[SELLER, agentsById.get(SELLER_AGENT.id)]]);

  const store = {
    createPayment: mock.fn(async (payment) => {
      payments.push({ ...payment });
      return { ...payment };
    }),
    getPayment: mock.fn(async (id) => {
      const payment = payments.find((candidate) => candidate.id === id);
      return payment ? { ...payment } : null;
    }),
    updatePayment: mock.fn(async (id, updates) => {
      const index = payments.findIndex((candidate) => candidate.id === id);
      if (index < 0) return null;
      payments[index] = { ...payments[index], ...updates };
      return { ...payments[index] };
    }),
    listPayments: mock.fn(async () => payments.map((payment) => ({ ...payment }))),
    sumPayments: mock.fn(async () => 0),
    createPaymentRequest: mock.fn(async (request) => {
      paymentRequests.set(request.id, { ...request });
      return { ...request };
    }),
    getPaymentRequest: mock.fn(async (id) => {
      const request = paymentRequests.get(id);
      return request ? { ...request } : null;
    }),
    updatePaymentRequest: mock.fn(async (id, updates) => {
      const request = paymentRequests.get(id);
      if (!request) return null;
      const updated = { ...request, ...updates };
      paymentRequests.set(id, updated);
      return { ...updated };
    }),
    listPaymentRequests: mock.fn(async () => [...paymentRequests.values()].map((request) => ({ ...request }))),
    createQuote: mock.fn(async (quote) => {
      quotes.set(quote.id, { ...quote });
      return { ...quote };
    }),
    getQuote: mock.fn(async (id) => {
      const quote = quotes.get(id);
      return quote ? { ...quote } : null;
    }),
    updateQuote: mock.fn(async (id, updates) => {
      const quote = quotes.get(id);
      if (!quote) return null;
      const updated = { ...quote, ...updates };
      quotes.set(id, updated);
      return { ...updated };
    }),
    listQuotes: mock.fn(async () => [...quotes.values()].map((quote) => ({ ...quote }))),
  };

  return {
    commerce: {
      a2a: () => store,
      x402: () => ({
        getAgent: mock.fn(async (id) => agentsById.get(id) || null),
        getAgentByWallet: mock.fn(async (wallet) => agentsByWallet.get(wallet) || null),
      }),
    },
    paymentRequests,
    quotes,
    payments,
  };
}

describe('A2A network routing', () => {
  let commerce;
  let paymentRequests;
  let quotes;
  let payments;
  let buyerSvc;
  let sellerSvc;

  beforeEach(() => {
    ({ commerce, paymentRequests, quotes, payments } = createMockCommerce());
    commerce._finalityTracker = createFinalityTracker();
    buyerSvc = createA2AService(commerce, {
      agentId: 'buyer',
      walletAddress: BUYER,
      defaultAsset: 'USDC',
      defaultNetwork: 'set_chain',
    });
    sellerSvc = createA2AService(commerce, {
      agentId: 'seller',
      walletAddress: SELLER,
      defaultAsset: 'USDC',
      defaultNetwork: 'set_chain',
      receiveAddressForNetwork: async (network) => (network === 'zcash' ? SELLER_ZCASH : SELLER),
    });
  });

  afterEach(() => {
    global.fetch = ORIGINAL_FETCH;
  });

  it('stores and exposes the requested payment network', async () => {
    const result = await buyerSvc.requestPayment({
      amount: 0.001,
      description: 'Bitcoin settlement',
      asset: 'BTC',
      network: 'bitcoin',
    });

    const stored = paymentRequests.get(result.request.id);
    assert.deepEqual(stored.accepted_networks, ['bitcoin']);
    assert.equal(JSON.parse(stored.metadata).requester_payment_address, BUYER);
    assert.equal(result.request.network, 'bitcoin');
    assert.deepEqual(result.request.acceptedNetworks, ['bitcoin']);
    assert.equal(result.request.requesterPaymentAddress, BUYER);
  });

  it('routes direct payments by agent identity wallet to the network payout address', async () => {
    const result = await buyerSvc.pay({
      to: SELLER,
      amount: 0.005,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Direct settlement',
    });

    assert.equal(result.success, true);
    assert.equal(payments.length, 1);
    assert.equal(payments[0].recipient_address, SELLER_BITCOIN);
    assert.equal(payments[0].recipient_agent_id, SELLER_AGENT.id);
  });

  it('uses the requester payout address when paying a request', async () => {
    const request = await sellerSvc.requestPayment({
      amount: 0.25,
      description: 'Shielded settlement',
      asset: 'ZEC',
      network: 'zcash',
      allowPartial: true,
    });

    const result = await buyerSvc.payRequest(request.request.id);

    assert.equal(result.success, true);
    assert.equal(payments.length, 1);
    assert.equal(payments[0].network, 'zcash');
    assert.equal(payments[0].asset, 'ZEC');
    assert.equal(payments[0].recipient_address, SELLER_ZCASH);
    assert.equal(JSON.parse(paymentRequests.get(request.request.id).metadata).requester_payment_address, SELLER_ZCASH);
  });

  it('stores and exposes the requested quote network', async () => {
    const result = await buyerSvc.requestQuote({
      seller: SELLER_AGENT,
      items: [{ description: 'Agent service' }],
      asset: 'BTC',
      network: 'bitcoin',
    });

    const stored = quotes.get(result.quote.id);
    assert.deepEqual(stored.accepted_networks, ['bitcoin']);
    assert.equal(JSON.parse(stored.metadata).seller_payment_address, SELLER_BITCOIN);
    assert.equal(result.quote.network, 'bitcoin');
    assert.deepEqual(result.quote.acceptedNetworks, ['bitcoin']);
    assert.equal(result.quote.sellerPaymentAddress, SELLER_BITCOIN);
  });

  it('uses the seller payout address when accepting a quote', async () => {
    const requested = await buyerSvc.requestQuote({
      seller: SELLER_AGENT,
      items: [{ description: 'Compute time' }],
      asset: 'BTC',
      network: 'bitcoin',
    });

    await sellerSvc.provideQuote(requested.quote.id, { total: 0.01 });
    const accepted = await buyerSvc.acceptQuote(requested.quote.id);

    assert.equal(accepted.success, true);
    assert.equal(accepted.quote.network, 'bitcoin');
    assert.equal(payments.length, 1);
    assert.equal(payments[0].network, 'bitcoin');
    assert.equal(payments[0].asset, 'BTC');
    assert.equal(payments[0].recipient_address, SELLER_BITCOIN);
  });

  it('exposes stored settlement metadata when listing payments', async () => {
    const result = await buyerSvc.pay({
      to: SELLER,
      amount: 0.005,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Confirmed settlement',
    });

    const stored = payments[0];
    stored.status = 'completed';
    stored.tx_hash = 'a'.repeat(64);
    stored.block_number = 123;
    stored.completed_at = new Date().toISOString();
    stored.metadata = JSON.stringify({
      explorer_url: 'https://example.test/tx/' + 'a'.repeat(64),
      confirmations: 6,
      chain_id: 'bitcoin',
      simulated: false,
    });

    const listed = await buyerSvc.getPayments({ sent: true });

    assert.equal(result.success, true);
    assert.equal(listed.length, 1);
    assert.equal(listed[0].txHash, 'a'.repeat(64));
    assert.equal(listed[0].blockNumber, 123);
    assert.equal(listed[0].explorerUrl, 'https://example.test/tx/' + 'a'.repeat(64));
    assert.equal(listed[0].confirmations, 6);
    assert.equal(listed[0].chainId, 'bitcoin');
    assert.equal(listed[0].simulated, false);
  });

  it('refreshes a bitcoin payment from live chain status', async () => {
    const result = await buyerSvc.pay({
      to: SELLER,
      amount: 0.005,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Refreshable settlement',
    });

    const stored = payments[0];
    stored.status = 'submitted';
    stored.tx_hash = 'b'.repeat(64);
    stored.block_number = null;
    stored.metadata = JSON.stringify({
      explorer_url: 'https://mempool.space/tx/' + 'b'.repeat(64),
      chain_id: 'bitcoin',
    });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${stored.tx_hash}/status`)) {
        return {
          ok: true,
          async json() {
            return {
              confirmed: true,
              block_height: 100,
              block_time: 1_710_000_100,
            };
          },
          async text() {
            return JSON.stringify({
              confirmed: true,
              block_height: 100,
              block_time: 1_710_000_100,
            });
          },
        };
      }

      if (url.endsWith('/blocks/tip/height')) {
        return {
          ok: true,
          async json() {
            return 105;
          },
          async text() {
            return '105';
          },
        };
      }

      throw new Error(`Unexpected fetch: ${url}`);
    };

    const refreshed = await buyerSvc.refreshPayment(result.payment.id);

    assert.equal(refreshed.success, true);
    assert.equal(refreshed.refreshed, true);
    assert.equal(refreshed.payment.status, 'completed');
    assert.equal(refreshed.payment.blockNumber, 100);
    assert.equal(refreshed.payment.confirmations, 6);
    assert.equal(refreshed.payment.chainId, 'bitcoin');
    assert.equal(refreshed.onChain.final, true);
    assert.equal(refreshed.onChain.requiredConfirmations, 6);
    assert.equal(refreshed.finality?.state, 'final');
    assert.equal(refreshed.finality?.confirmations, 6);

    assert.equal(payments[0].status, 'completed');
    assert.equal(payments[0].block_number, 100);
    assert.equal(JSON.parse(payments[0].metadata).confirmations, 6);
    assert.equal(commerce._finalityTracker.getSettlementStatus(result.payment.id).state, 'final');
  });

  it('bulk-refreshes submitted bitcoin payments when listing payments', async () => {
    const first = await buyerSvc.pay({
      to: SELLER,
      amount: 0.005,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Pending settlement one',
    });
    const second = await buyerSvc.pay({
      to: SELLER,
      amount: 0.006,
      asset: 'BTC',
      network: 'bitcoin',
      memo: 'Pending settlement two',
    });

    payments[0].status = 'submitted';
    payments[0].tx_hash = 'c'.repeat(64);
    payments[0].metadata = JSON.stringify({ chain_id: 'bitcoin' });
    payments[1].status = 'submitted';
    payments[1].tx_hash = 'd'.repeat(64);
    payments[1].metadata = JSON.stringify({ chain_id: 'bitcoin' });

    global.fetch = async (input) => {
      const url = typeof input === 'string' ? input : input.url;
      if (url.endsWith(`/tx/${payments[0].tx_hash}/status`)) {
        return {
          ok: true,
          async json() {
            return { confirmed: true, block_height: 120, block_time: 1_710_000_120 };
          },
          async text() {
            return JSON.stringify({ confirmed: true, block_height: 120, block_time: 1_710_000_120 });
          },
        };
      }
      if (url.endsWith(`/tx/${payments[1].tx_hash}/status`)) {
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
            return 125;
          },
          async text() {
            return '125';
          },
        };
      }
      throw new Error(`Unexpected fetch: ${url}`);
    };

    const listed = await buyerSvc.getPayments({ sent: true, refreshOnChain: true });

    assert.equal(first.success, true);
    assert.equal(second.success, true);
    assert.equal(listed.length, 2);
    const refreshed = listed.find((payment) => payment.id === payments[0].id);
    const pending = listed.find((payment) => payment.id === payments[1].id);
    assert.equal(refreshed.status, 'completed');
    assert.equal(refreshed.confirmations, 6);
    assert.equal(refreshed.finality?.state, 'final');
    assert.equal(pending.status, 'submitted');
    assert.equal(pending.confirmations, 0);
    assert.equal(pending.finality?.state, 'unconfirmed');
  });

  it('preserves stored zcash confirmation state when live status lookup is unavailable', async () => {
    const result = await buyerSvc.pay({
      to: SELLER,
      amount: 0.005,
      asset: 'ZEC',
      network: 'zcash',
      memo: 'Shielded pending settlement',
    });

    payments[0].status = 'submitted';
    payments[0].tx_hash = 'e'.repeat(64);
    payments[0].block_number = 300;
    payments[0].metadata = JSON.stringify({
      chain_id: 'zcash',
      confirmations: 3,
      explorer_url: 'https://zcashblockexplorer.com/tx/' + 'e'.repeat(64),
    });

    const listed = await buyerSvc.getPayments({ sent: true, refreshOnChain: true });

    assert.equal(result.success, true);
    assert.equal(listed.length, 1);
    assert.equal(listed[0].status, 'submitted');
    assert.equal(listed[0].confirmations, 3);
    assert.equal(listed[0].finality?.state, 'confirming');
    assert.equal(payments[0].status, 'submitted');
    assert.equal(JSON.parse(payments[0].metadata).confirmations, 3);
  });

  it('builds a mixed-asset payment summary with per-network breakdowns', async () => {
    const now = new Date().toISOString();
    payments.push(
      {
        id: 'pay-btc-sent',
        status: 'completed',
        sender_agent_id: 'buyer',
        sender_address: BUYER,
        recipient_agent_id: SELLER_AGENT.id,
        recipient_address: SELLER_BITCOIN,
        amount: 1_000_000,
        amount_decimal: 0.01,
        asset: 'BTC',
        network: 'bitcoin',
        memo: 'BTC outgoing',
        created_at: now,
        updated_at: now,
        completed_at: now,
      },
      {
        id: 'pay-btc-received',
        status: 'completed',
        sender_agent_id: SELLER_AGENT.id,
        sender_address: SELLER,
        recipient_agent_id: 'buyer',
        recipient_address: BUYER,
        amount: 400_000,
        amount_decimal: 0.004,
        asset: 'BTC',
        network: 'bitcoin',
        memo: 'BTC incoming',
        created_at: now,
        updated_at: now,
        completed_at: now,
      },
      {
        id: 'pay-zec-sent',
        status: 'completed',
        sender_agent_id: 'buyer',
        sender_address: BUYER,
        recipient_agent_id: SELLER_AGENT.id,
        recipient_address: SELLER_ZCASH,
        amount: 125_000_000,
        amount_decimal: 1.25,
        asset: 'ZEC',
        network: 'zcash',
        memo: 'ZEC outgoing',
        created_at: now,
        updated_at: now,
        completed_at: now,
      },
    );

    const balance = await buyerSvc.getBalance();

    assert.ok(Math.abs(balance.totalSent - 1.26) < 1e-12);
    assert.ok(Math.abs(balance.totalReceived - 0.004) < 1e-12);
    assert.ok(Math.abs(balance.netFlow + 1.256) < 1e-12);
    assert.equal(balance.aggregateTotalsMeaningful, false);
    assert.equal(balance.aggregateAsset, null);
    assert.deepEqual(balance.assets, ['BTC', 'ZEC']);
    assert.equal(balance.paymentCountSent, 2);
    assert.equal(balance.paymentCountReceived, 1);
    assert.equal(balance.paymentCount, 3);
    assert.equal(balance.summarySource, 'list_payments_fallback');
    assert.ok(Math.abs(balance.breakdownByAsset.BTC.totalSent - 0.01) < 1e-12);
    assert.ok(Math.abs(balance.breakdownByAsset.BTC.totalReceived - 0.004) < 1e-12);
    assert.ok(Math.abs(balance.breakdownByAsset.BTC.netFlow + 0.006) < 1e-12);
    assert.ok(Math.abs(balance.breakdownByAsset.BTC.networks.bitcoin.totalSent - 0.01) < 1e-12);
    assert.ok(Math.abs(balance.breakdownByAsset.BTC.networks.bitcoin.totalReceived - 0.004) < 1e-12);
    assert.ok(Math.abs(balance.breakdownByAsset.ZEC.totalSent - 1.25) < 1e-12);
    assert.equal(balance.breakdownByAsset.ZEC.totalReceived, 0);
    assert.ok(Math.abs(balance.breakdownByAsset.ZEC.networks.zcash.totalSent - 1.25) < 1e-12);
  });
});
