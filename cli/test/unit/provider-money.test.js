import { beforeEach, test } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { formatMoney, paymentAmount, fromMinorUnits } from '../../src/tools/providers/money.js';
import {
  __resetPaymentProviderState,
  createPaymentIntent,
  capturePaymentIntent,
  refundPaymentIntent,
  createPaymentSettlementBatch,
  reconcilePaymentProvider,
  ingestPaymentProviderWebhook,
  getPaymentIntent,
} from '../../src/tools/providers/payments.js';
import {
  __resetTaxProviderState,
  calculateTaxQuote,
  commitTaxTransaction,
  ingestTaxProviderWebhook,
} from '../../src/tools/providers/tax.js';
import {
  __resetShippingProviderState,
  quoteShippingRates,
  createShippingLabel,
} from '../../src/tools/providers/shipping.js';
import { paymentTools } from '../../src/tools/payments.js';
import { taxTools } from '../../src/tools/tax.js';

beforeEach(() => {
  __resetPaymentProviderState();
  __resetTaxProviderState();
  __resetShippingProviderState();
});

test('decimal formatting preserves large amounts and rounds explicit computed values half away from zero', () => {
  assert.equal(formatMoney('9007199254740993.25'), '9007199254740993.25');
  assert.equal(formatMoney('1.005'), '1.01');
  assert.equal(formatMoney('-1.005'), '-1.01');
  assert.equal(formatMoney('1.5', 'JPY'), '2');
  assert.equal(formatMoney('1.0005', 'KWD'), '1.001');
  assert.equal(fromMinorUnits('900719925474099325', 'USD'), '9007199254740993.25');
  assert.equal(fromMinorUnits('1234', 'KWD'), '1.234');
  assert.equal(fromMinorUnits('1234', 'JPY'), '1234');
});

test('monetary instructions reject malformed, unsafe, negative, zero, and excess precision inputs', () => {
  for (const value of [
    '12.34garbage',
    '',
    ' 12',
    '12 ',
    '1e3',
    'NaN',
    'Infinity',
    null,
    {},
    true,
    NaN,
    Infinity,
    9007199254740992,
  ]) {
    assert.throws(() => paymentAmount(value), /Invalid|Unsafe/);
  }
  for (const value of ['0', '-1']) assert.throws(() => paymentAmount(value), /positive/);
  assert.throws(() => paymentAmount('1.005'), /precision/);
  assert.throws(() => paymentAmount('1.1', 'JPY'), /precision/);
  assert.throws(() => paymentAmount(90071992547409.92), /Unsafe/);
  assert.throws(() => paymentAmount('1', 'UNKNOWN'), /Unsupported/);
  assert.throws(() => paymentAmount('1', 'ETH'), /denomination/);
  assert.throws(() => fromMinorUnits('1.2'), /integer/);
  assert.equal(paymentAmount('1.2300'), '1.23');
  assert.equal(paymentAmount(12.34), '12.34'); // existing numeric callers
});

test('large partial captures, refunds, settlement, and reconciliation preserve every cent', () => {
  const { intent } = createPaymentIntent({ amount: '9007199254740993.25' });
  capturePaymentIntent({ intentId: intent.id, amount: '9007199254740993.24' });
  const captured = capturePaymentIntent({ intentId: intent.id, amount: '0.01' });
  assert.equal(captured.intent.capturedAmount, '9007199254740993.25');
  refundPaymentIntent({ intentId: intent.id, amount: '0.01' });
  const batch = createPaymentSettlementBatch({ intentIds: [intent.id] });
  assert.equal(batch.batch.totalSettledAmount, '9007199254740993.24');
  const report = reconcilePaymentProvider();
  assert.equal(report.summary.outstandingAmount, '0.00');
  assert.equal(report.summary.settledAmount, '9007199254740993.24');
  assert.equal(report.summary.balancedCount, 1);
});

test('mixed currencies have separate totals and no misleading aggregate', () => {
  createPaymentIntent({ amount: '100', currency: 'JPY', captureMethod: 'automatic' });
  createPaymentIntent({ amount: '1.234', currency: 'KWD', captureMethod: 'automatic' });
  const { batch } = createPaymentSettlementBatch();
  assert.equal(batch.totalSettledAmount, null);
  assert.equal(batch.currency, null);
  assert.deepEqual(
    Object.fromEntries(batch.totalsByCurrency.map((entry) => [entry.currency, entry.amount])),
    { JPY: '100', KWD: '1.234' },
  );
  const { summary, reconciliation } = reconcilePaymentProvider();
  assert.equal(summary.capturedAmount, null);
  assert.equal(
    summary.totalsByCurrency.find((entry) => entry.currency === 'KWD').settledAmount,
    '1.234',
  );
  assert.equal(reconciliation.find((entry) => entry.currency === 'JPY').outstandingAmount, '0');
});

test('full-refund and capture retries retain their operation identity after terminal status', () => {
  const { intent } = createPaymentIntent({ amount: '10.00' });
  const capture = capturePaymentIntent({ intentId: intent.id, idempotencyKey: 'capture-1' });
  const refund = refundPaymentIntent({ intentId: intent.id, idempotencyKey: 'refund-1' });
  const replay = refundPaymentIntent({ intentId: intent.id, idempotencyKey: 'refund-1' });
  assert.equal(replay.idempotent, true);
  assert.equal(replay.refund.id, refund.refund.id);
  assert.equal(
    capturePaymentIntent({ intentId: intent.id, idempotencyKey: 'capture-1' }).capture.id,
    capture.capture.id,
  );
  assert.equal(getPaymentIntent(intent.id).refunds.length, 1);
});

test('idempotency keys bind normalized amounts and business parameters', () => {
  const { intent } = createPaymentIntent({ amount: '10', idempotencyKey: 'create-1' });
  assert.equal(
    createPaymentIntent({ amount: '10.00', idempotencyKey: 'create-1' }).idempotent,
    true,
  );
  assert.throws(
    () => createPaymentIntent({ amount: '11', idempotencyKey: 'create-1' }),
    /conflicts/,
  );
  assert.throws(
    () => createPaymentIntent({ amount: '10', currency: 'EUR', idempotencyKey: 'create-1' }),
    /conflicts/,
  );
  capturePaymentIntent({ intentId: intent.id, amount: '5', idempotencyKey: 'capture-1' });
  assert.throws(
    () => capturePaymentIntent({ intentId: intent.id, amount: '6', idempotencyKey: 'capture-1' }),
    /conflicts/,
  );
  refundPaymentIntent({
    intentId: intent.id,
    amount: '5',
    reason: 'return',
    idempotencyKey: 'refund-1',
  });
  assert.throws(
    () =>
      refundPaymentIntent({
        intentId: intent.id,
        amount: '5',
        reason: 'duplicate',
        idempotencyKey: 'refund-1',
      }),
    /conflicts/,
  );
});

test('webhook minor units respect currency and invalid values never fall back to a full capture', () => {
  const { intent } = createPaymentIntent({ amount: '1.234', currency: 'KWD' });
  const event = {
    eventId: 'event-1',
    eventType: 'payment.captured',
    payload: { intentId: intent.id, amount_received: '1234' },
  };
  const captured = ingestPaymentProviderWebhook(event);
  assert.equal(captured.intent.capturedAmount, '1.234');
  assert.equal(ingestPaymentProviderWebhook(event).idempotent, true);
  const other = createPaymentIntent({ amount: '100' }).intent;
  for (const amount of ['garbage', '', '0', '-1', '1.2']) {
    assert.throws(() =>
      ingestPaymentProviderWebhook({
        eventType: 'payment.captured',
        payload: { intentId: other.id, amount_minor: amount },
      }),
    );
    assert.equal(getPaymentIntent(other.id).capturedAmount, '0.00');
  }
});

test('tax totals use exact line rounding, including large totals and digital category rates', () => {
  const { quote } = calculateTaxQuote({
    lineItems: [
      { unitPrice: '9007199254740993.25', quantity: 1, taxCategory: 'exempt' },
      { unitPrice: '1.00', quantity: 1, taxCategory: 'digital' },
    ],
    shippingAddress: { country: 'US', state: 'CA' },
    shippingAmount: '0.10',
  });
  assert.equal(quote.subtotal, '9007199254740994.25');
  assert.equal(quote.totalTax, '0.09');
  assert.equal(quote.total, '9007199254740994.44');
  assert.equal(quote.lineItems[1].taxRate, 0.0825);
  const half = calculateTaxQuote({
    lineItems: [{ unitPrice: '0.10', quantity: 1 }],
    shippingAddress: { country: 'US' },
  }).quote;
  assert.equal(half.totalTax, '0.01');
  for (const quantity of [0, -1, 1.5, '1']) {
    assert.throws(
      () => calculateTaxQuote({ lineItems: [{ unitPrice: '1', quantity }] }),
      /Quantity/,
    );
  }
});

test('tax adjustment webhooks preserve currency scale and reject malformed amounts without mutation', () => {
  const { quote } = calculateTaxQuote({
    currency: 'KWD',
    lineItems: [{ unitPrice: '1.234', quantity: 1 }],
    shippingAddress: { country: 'US', state: 'CA' },
  });
  const { transaction } = commitTaxTransaction({ quoteId: quote.id });
  const result = ingestTaxProviderWebhook({
    eventType: 'tax.adjusted',
    payload: { transactionId: transaction.id, total_tax_minor: '123', amount_minor: '1357' },
  });
  assert.equal(result.transaction.totalTax, '0.123');
  assert.equal(result.transaction.total, '1.357');
  assert.throws(
    () =>
      ingestTaxProviderWebhook({
        eventType: 'tax.adjusted',
        payload: { transactionId: transaction.id, amount: '12garbage' },
      }),
    /Invalid/,
  );
});

test('shipping quote and purchased label agree at the requested currency scale', () => {
  for (const currency of ['USD', 'JPY', 'KWD']) {
    const { rates } = quoteShippingRates({
      currency,
      parcels: [{ weightGrams: 1234 }],
      originAddress: { country: 'US', state: 'CA' },
      destinationAddress: { country: 'US', state: 'CA' },
    });
    const { label } = createShippingLabel({ rateId: rates[0].rateId });
    assert.equal(label.amount, rates[0].amount);
    assert.equal(label.currency, currency);
    assert.equal(rates[0].amount, { USD: '6.70', JPY: '7', KWD: '6.704' }[currency]);
  }
});

test('agent tool schemas expose exact decimal inputs while preserving preview gates', async () => {
  const payment = paymentTools.find((tool) => tool.name === 'create_payment_intent');
  const params = z.object(payment.inputSchema).parse({ amount: '9007199254740993.25' });
  const preview = await payment.handler({ params, allowApply: false });
  assert.equal(preview.success, false);
  const created = await payment.handler({ params, allowApply: true });
  assert.equal(created.intent.amount, params.amount);
  const tax = taxTools.find((tool) => tool.name === 'calculate_tax_quote');
  assert.equal(
    z
      .object(tax.inputSchema)
      .parse({
        items: [{ unitPrice: '9007199254740993.25', quantity: 1 }],
        shippingAddress: { country: 'US' },
      }).items[0].unitPrice,
    params.amount,
  );
});
