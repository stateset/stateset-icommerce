// The Node binding against the shared semantic corpus.
//
// `bindings/test-vectors/semantics-v1.json` pins the MEANING of money across
// every binding: currency scale and its enforcement, the exact decimal a
// value renders to, the published tax tables, and which inputs must be
// refused. The Rust test at `crates/stateset-embedded/tests/semantics_vectors.rs`
// keeps that file honest against the engine, so conforming to it means
// conforming to the engine.
//
// This file asserts the categories the Node surface can reach today and
// DECLARES the ones it cannot, with the declaration checked against the
// corpus — so a new category cannot slip in unnoticed, and a silent skip
// cannot hide coverage standing still.

'use strict';

const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const { Commerce, Tax } = require('../index.js');

const CORPUS_PATH = path.join(__dirname, '..', '..', 'test-vectors', 'semantics-v1.json');

function corpus() {
  const doc = JSON.parse(fs.readFileSync(CORPUS_PATH, 'utf8'));
  assert.equal(doc.version, 1, 'corpus version must be 1');
  return doc;
}

function rows(category) {
  return corpus().categories[category].rows;
}

// Categories this binding cannot assert yet, with the reason.
const NOT_REACHABLE = {};

const ASSERTED = new Set([
  'decimal_render',
  'money_scale_enforced',
  'rejected_inputs',
  'accepted_inputs',
  'canadian_tax_rates',
  'currency_decimals',
]);

test('every corpus category is either asserted or declared unreachable', () => {
  const present = new Set(Object.keys(corpus().categories));
  const accounted = new Set([...ASSERTED, ...Object.keys(NOT_REACHABLE)]);
  assert.deepEqual(
    [...present].sort(),
    [...accounted].sort(),
    'a corpus category is unaccounted for — assert it or declare why not',
  );
});

async function customerFor(commerce) {
  return commerce.customers.create({
    email: 'vectors@example.com',
    firstName: 'V',
    lastName: 'Ectors',
  });
}

test('decimal_render survives the boundary', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await customerFor(commerce);

  for (const row of rows('decimal_render')) {
    if (row.op !== 'add' || !row.money_scale_ok) continue;
    // One order line per operand at unit quantity, so the engine's own
    // arithmetic produces the total a caller reads back. `unitPriceExact`
    // carries the amount losslessly in; `totalAmountExact` carries it out.
    const items = row.operands.map((amount, i) => ({
      sku: `SKU-${i}`,
      name: `line-${i}`,
      quantity: 1,
      unitPrice: Number(amount),
      unitPriceExact: amount,
    }));
    const order = await commerce.orders.create({ customerId: customer.id, items });
    const want = row.operands
      .reduce((sum, a) => sum + Math.round(Number(a) * 100), 0);
    assert.equal(
      Math.round(Number(order.totalAmountExact) * 100),
      want,
      `${row.id}: totalAmountExact is ${order.totalAmountExact}`,
    );
    // The exact field is a STRING; a binding that routed money through a
    // float could not carry the scale back out.
    assert.equal(typeof order.totalAmountExact, 'string', `${row.id}: exact is a string`);
  }
});

test('money scale is enforced per currency', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await customerFor(commerce);

  for (const row of rows('money_scale_enforced')) {
    if (row.currency !== 'USD') continue; // orders here use the store default
    const items = [
      {
        sku: 'SKU-1',
        name: 'Widget',
        quantity: 1,
        unitPrice: Number(row.amount),
        unitPriceExact: row.amount,
      },
    ];
    if (row.must_reject) {
      await assert.rejects(
        () => commerce.orders.create({ customerId: customer.id, items }),
        (err) => /decimal places/i.test(err.message),
        `${row.id}: ${row.amount} must be refused for ${row.currency}`,
      );
    } else {
      const order = await commerce.orders.create({ customerId: customer.id, items });
      assert.equal(
        Number(order.totalAmountExact),
        Number(row.amount),
        `${row.id}: ${row.amount} must be accepted`,
      );
    }
  }
});

test('decimal_render multiplication survives the boundary', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await customerFor(commerce);

  for (const row of rows('decimal_render')) {
    if (row.op !== 'mul' || !row.money_scale_ok) continue;
    // A quantity times a price is a different path from a sum. 19.99 x 2 is
    // the classic case: the price has no exact binary representation, so a
    // binding that multiplies in floating point lands near 39.98, not on it.
    const [price, quantity] = row.operands;
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [
        {
          sku: 'SKU-1',
          name: 'Widget',
          quantity: Number(quantity),
          unitPrice: Number(price),
          unitPriceExact: price,
        },
      ],
    });
    assert.equal(
      order.totalAmountExact,
      row.expected,
      `${row.id}: ${quantity} x ${price} came back as ${order.totalAmountExact}`,
    );
  }
});

test('rejected_inputs are refused, and refusal writes nothing', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await customerFor(commerce);
  const item = { sku: 'SKU-1', name: 'Widget', quantity: 1, unitPrice: 10, unitPriceExact: '10.00' };

  for (const row of rows('rejected_inputs')) {
    let attempt;
    if (row.kind === 'currency') {
      attempt = () =>
        commerce.orders.create({ customerId: customer.id, currency: row.value, items: [item] });
    } else if (row.kind === 'uuid') {
      attempt = () => commerce.orders.create({ customerId: row.value, items: [item] });
    } else {
      continue; // timestamps and dates have no order-path field to carry them
    }
    const before = (await commerce.orders.list()).length;
    await assert.rejects(attempt, `${row.id}: ${JSON.stringify(row.value)} must be refused`);
    assert.equal(
      (await commerce.orders.list()).length,
      before,
      `${row.id}: a refused input must not write an order`,
    );
  }
});

test('accepted_inputs still work after strict parsing', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await customerFor(commerce);
  for (const row of rows('accepted_inputs')) {
    if (row.kind !== 'currency') continue;
    const order = await commerce.orders.create({
      customerId: customer.id,
      currency: row.value,
      items: [{ sku: 'SKU-1', name: 'Widget', quantity: 1, unitPrice: 10, unitPriceExact: '10.00' }],
    });
    assert.equal(order.currency, row.normalizes_to, `${row.id}: normalized currency`);
  }
});

test('canadian_tax_rates match the corpus', () => {
  // The same table the Rust test pins. Quebec's rates were ten times too
  // large until Sep 2026; reintroducing that fails this in every binding.
  const fields = { gst: 'gstRate', pst: 'pstRate', hst: 'hstRate', qst: 'qstRate', total: 'totalRate' };
  for (const row of rows('canadian_tax_rates')) {
    const info = Tax.getCanadianTaxInfo(row.province);
    assert.ok(info, `${row.province} is in the table`);
    for (const [key, prop] of Object.entries(fields)) {
      const want = row[key];
      const got = info[prop];
      if (want === null) {
        assert.ok(got === undefined || got === null, `${row.id}: ${key} should be absent, got ${got}`);
      } else {
        assert.ok(
          Math.abs(got - Number(want)) < 1e-12,
          `${row.id}: ${key} is ${got}, corpus says ${want}`,
        );
      }
    }
  }
});

test('currency_decimals match the corpus', () => {
  const commerce = new Commerce(':memory:');
  for (const row of rows('currency_decimals')) {
    assert.equal(
      commerce.currency.decimalPlaces(row.code),
      row.decimals,
      `${row.id}: ${row.code} decimal places`,
    );
  }
});
