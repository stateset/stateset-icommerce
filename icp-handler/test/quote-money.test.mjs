import test from 'node:test';
import assert from 'node:assert/strict';
import { priceDemoQuote } from '../src/quote-money.mjs';
const line = (amount, quantity = 1) => ({
  sku: 'SKU',
  quantity,
  unit_price: { amount, currency: 'USDC' },
});
test('quote totals preserve cents above the JavaScript safe-integer boundary', () => {
  const result = priceDemoQuote([line('9007199254740993.01')], {
    amount: '9457559217478042.66',
    currency: 'USDC',
  });
  assert.equal(result.amount, '9457559217478042.66');
  assert.equal(result.exceedsMaximum, false);
  assert.equal(
    priceDemoQuote([line('9007199254740993.01')], {
      amount: '9457559217478042.65',
      currency: 'USDC',
    }).exceedsMaximum,
    true,
  );
});
test('round half-up once at the quote total, reject floats and mixed currencies', () => {
  assert.equal(
    priceDemoQuote([line('29.99', 2)], { amount: '100', currency: 'USDC' }).amount,
    '62.98',
  );
  assert.equal(priceDemoQuote([line('0.10')], { amount: '1', currency: 'USDC' }).amount, '0.11');
  assert.throws(() => priceDemoQuote([line(0.1)], { amount: '1', currency: 'USDC' }));
  assert.throws(() => priceDemoQuote([line('1')], { amount: '1', currency: 'USD' }));
});
