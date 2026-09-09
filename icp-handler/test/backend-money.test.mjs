import test from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, randomUUID } from 'node:crypto';
import { amount } from '../src/quote-money.mjs';
import {
  stubPayoutRequest,
  _seedSellerBalance,
  stubQuoteRequest,
  stubQuote,
  stubSubscriptionAuthorize,
  stubReturnAuthorize,
  stubInventoryQuery,
} from '../src/backend-stub.mjs';
import { reserveInventory, availableInventory } from '../src/state.mjs';

const { privateKey } = generateKeyPairSync('ed25519');
const payout = (seller, value, currency = 'USDC') => ({
  intent_id: 'intent:test',
  seller,
  platform: 'platform:test',
  amount: { amount: value, currency },
});

test('large payouts conserve the exact debit across approved funds and rounded fees', () => {
  const seller = randomUUID();
  const balance = '9007199254740993.01';
  _seedSellerBalance(seller, balance);
  const result = stubPayoutRequest(payout(seller, '9007199254740993'), privateKey);
  assert.equal(result.ok, true);
  const auth = result.authorization;
  assert.equal(auth.available_balance.amount, balance);
  const sum =
    amount(auth.approved_amount.amount) +
    auth.fees.reduce((n, fee) => n + amount(fee.amount.amount), 0n);
  assert.equal(sum, amount('9007199254740993'));
  const remainder = stubPayoutRequest(payout(seller, '0.01'), privateKey);
  assert.equal(remainder.authorization.available_balance.amount, '0.01');
  assert.equal(
    stubPayoutRequest(payout(seller, '0.01'), privateKey).error.code,
    'policy.payout.insufficient_balance',
  );
});

test('payout validation cannot credit balances or bypass an authority cap', () => {
  const seller = randomUUID();
  _seedSellerBalance(seller, '10');
  for (const value of [-1, 0.1, '-1', 'NaN', 'Infinity', '1e2', '', '0']) {
    assert.equal(
      stubPayoutRequest(payout(seller, value), privateKey).error.code,
      'format.invalid_money',
    );
  }
  assert.equal(stubPayoutRequest(payout(seller, '1', 'USD'), privateKey).ok, false);
  const intent = payout(seller, '1.000000000000000001');
  intent.principal_binding = { authority: { max_per_payout: { amount: '1', currency: 'USDC' } } };
  assert.equal(
    stubPayoutRequest(intent, privateKey).error.code,
    'policy.payout.exceeds_max_per_payout',
  );
  intent.principal_binding.authority.max_per_payout.currency = 'USD';
  assert.equal(stubPayoutRequest(intent, privateKey).error.code, 'format.invalid_money');
  assert.equal(
    stubPayoutRequest(payout(seller, '10'), privateKey).authorization.available_balance.amount,
    '10.00',
  );
});

test('a signing failure cannot consume seller funds', () => {
  const seller = randomUUID();
  _seedSellerBalance(seller, '10');
  assert.throws(() => stubPayoutRequest(payout(seller, '5'), 'invalid signing key'));
  assert.equal(
    stubPayoutRequest(payout(seller, '10'), privateKey).authorization.available_balance.amount,
    '10.00',
  );
});

test('payout fees conserve funds across sub-cent and cent rounding boundaries', () => {
  for (const value of [
    '0.000000000000000001',
    '0.005',
    '0.165',
    '0.50',
    '1.005',
    '16.665',
    '99.999',
  ]) {
    const seller = randomUUID();
    _seedSellerBalance(seller, value);
    const { authorization: auth } = stubPayoutRequest(payout(seller, value), privateKey);
    assert.equal(
      amount(auth.approved_amount.amount) +
        auth.fees.reduce((n, fee) => n + amount(fee.amount.amount), 0n),
      amount(value),
    );
  }
});

test('discounted unit prices retain precision and reproduce signed line totals', () => {
  const result = stubQuoteRequest(
    {
      intent_id: 'quote:test',
      merchant: 'merchant',
      items: [
        { sku: 'WIDGET-001', quantity: 100 },
        { sku: 'FASTENER-M6X20', quantity: 500 },
      ],
    },
    privateKey,
  );
  assert.equal(result.ok, true);
  assert.equal(result.proposal.items[0].unit_price.amount, '26.991');
  for (const line of result.proposal.items) {
    assert.equal(
      amount(line.unit_price.amount) * BigInt(line.quantity),
      amount(line.line_total.amount),
    );
  }
  assert.equal(result.proposal.total.amount, '2759.10');
  assert.equal(
    stubQuoteRequest({ items: [{ sku: 'WIDGET-001', quantity: -1 }] }, privateKey).ok,
    false,
  );
});

test('subscription caps compare all decimal places and reject invalid amounts', () => {
  for (const value of ['1000.000000000000000001', 'NaN', '-1', 10]) {
    assert.equal(
      stubSubscriptionAuthorize({ max_total_per_period: { amount: value } }, privateKey).ok,
      false,
    );
  }
});

test('proposal purchases enforce numeric ceilings, currency and merchant binding', () => {
  const { proposal } = stubQuoteRequest(
    {
      intent_id: 'quote:test',
      merchant: 'merchant',
      items: [{ sku: 'WIDGET-001', quantity: 100 }],
    },
    privateKey,
  );
  const intent = {
    intent_id: 'purchase:test',
    merchant: 'merchant',
    settler: 'settler',
    from_proposal_id: proposal.proposal_id,
    max_total: { amount: '2699.1', currency: 'USDC' },
  };
  assert.equal(stubQuote(intent, privateKey).ok, true);
  assert.equal(
    stubQuote({ ...intent, max_total: { amount: '3000', currency: 'USDC' } }, privateKey).ok,
    true,
  );
  for (const maximum of [
    { amount: '2699.099999999999999999', currency: 'USDC' },
    { amount: '3000', currency: 'USD' },
    { amount: 3000, currency: 'USDC' },
  ]) {
    assert.equal(stubQuote({ ...intent, max_total: maximum }, privateKey).ok, false);
  }
  assert.equal(stubQuote({ ...intent, merchant: 'another-merchant' }, privateKey).ok, false);
});

test('return authorization never rounds above a sub-cent cap', () => {
  const result = stubReturnAuthorize(
    {
      intent_id: 'return:test',
      merchant: 'merchant',
      original_settlement_id: 'settlement',
      items: [{ quantity: 1, reason: 'damaged' }],
      max_refund: { amount: '0.005', currency: 'USDC' },
      desired_outcome: 'refund',
    },
    privateKey,
  );
  assert.equal(result.authorization.refund.amount.amount, '0.005');
  assert.equal(stubReturnAuthorize({ items: [{ quantity: -1 }] }, privateKey).ok, false);
});

test('signed inventory snapshots reflect reservations and stock filters use live balances', () => {
  const quantity = availableInventory('GADGET-B');
  assert.ok(reserveInventory('0x' + randomUUID(), [{ sku: 'GADGET-B', quantity }]));
  const intent = { intent_id: 'inventory:test', merchant: 'merchant', skus: [{ sku: 'GADGET-B' }] };
  const result = stubInventoryQuery(intent, privateKey);
  assert.equal(result.snapshot.items[0].available_quantity, 0);
  const filtered = stubInventoryQuery({ ...intent, filters: { in_stock_only: true } }, privateKey);
  assert.equal(filtered.snapshot.items.length, 0);
});
