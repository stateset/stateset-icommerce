// The WASM binding against the shared semantic corpus.
//
// `bindings/test-vectors/semantics-v1.json` pins the MEANING of money across
// every binding, and the Rust test at
// `crates/stateset-embedded/tests/semantics_vectors.rs` keeps that file honest
// against the engine.
//
// This binding is a separate in-memory implementation, not the engine, with
// its own money type: a fixed two-decimal integer of minor units. So it
// asserts what that model can honour and DECLARES the rest with the reason.
// The declaration is checked against the corpus, so a new category cannot
// slip in unnoticed and a declared gap cannot quietly stand still.

'use strict';

const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const { Commerce, Tax } = require('../pkg-node/stateset_embedded_wasm.js');

const CORPUS_PATH = path.join(__dirname, '..', '..', 'test-vectors', 'semantics-v1.json');

function corpus() {
  const doc = JSON.parse(fs.readFileSync(CORPUS_PATH, 'utf8'));
  assert.equal(doc.version, 1, 'corpus version must be 1');
  return doc;
}

function rows(category) {
  return corpus().categories[category].rows;
}

// What this binding's money model cannot honour, and why.
const NOT_REACHABLE = {
  decimal_render:
    'money crosses the boundary as a JavaScript number -- the documented contract ' +
    '(README: `order.totalAmount // number`) -- so there is no exact string to assert',
  currency_decimals:
    'money is a fixed two-decimal integer of minor units (SCALE = 100), so zero- and ' +
    'eight-decimal currencies (JPY, BTC) cannot be represented; a scale accessor would ' +
    'describe a rule this binding does not follow',
};

const ASSERTED = new Set([
  'canadian_tax_rates',
  'rejected_inputs',
  'accepted_inputs',
  'money_scale_enforced',
]);

test('every corpus category is either asserted or declared unreachable', () => {
  const present = new Set(Object.keys(corpus().categories));
  const accounted = new Set([...ASSERTED, ...Object.keys(NOT_REACHABLE)]);
  assert.deepEqual(
    [...present].sort(),
    [...accounted].sort(),
    'a corpus category is unaccounted for -- assert it or declare why not',
  );
});

function customerFor(commerce) {
  return commerce.customers.create({ email: 'vectors@example.com', firstName: 'V', lastName: 'E' });
}

function item(customer, unitPrice) {
  return { productId: customer.id, sku: 'SKU-1', name: 'Widget', quantity: 1, unitPrice };
}

test('canadian_tax_rates match the corpus', () => {
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
        assert.ok(Math.abs(got - Number(want)) < 1e-12, `${row.id}: ${key} is ${got}, corpus says ${want}`);
      }
    }
  }
});

test('rejected_inputs are refused, and refusal writes nothing', () => {
  const commerce = new Commerce();
  const customer = customerFor(commerce);
  for (const row of rows('rejected_inputs')) {
    let attempt;
    if (row.kind === 'currency') {
      attempt = () =>
        commerce.orders.create({ customerId: customer.id, currency: row.value, items: [item(customer, 10)] });
    } else if (row.kind === 'uuid') {
      attempt = () => commerce.orders.create({ customerId: row.value, items: [item(customer, 10)] });
    } else {
      continue; // timestamps and dates have no order-path field to carry them
    }
    const before = commerce.orders.list().length;
    assert.throws(attempt, `${row.id}: ${JSON.stringify(row.value)} must be refused`);
    assert.equal(commerce.orders.list().length, before, `${row.id}: a refused input must not write`);
  }
});

test('accepted_inputs still work, and normalize', () => {
  const commerce = new Commerce();
  const customer = customerFor(commerce);
  for (const row of rows('accepted_inputs')) {
    if (row.kind !== 'currency') continue;
    const order = commerce.orders.create({
      customerId: customer.id,
      currency: row.value,
      items: [item(customer, 10)],
    });
    assert.equal(order.currency, row.normalizes_to, `${row.id}: normalized currency`);
  }
});

test('money scale is enforced rather than rounded', () => {
  // USD rows only: this binding's money is a fixed two-decimal integer, so it
  // cannot express JPY's zero-decimal rule (see NOT_REACHABLE.currency_decimals).
  const commerce = new Commerce();
  const customer = customerFor(commerce);
  for (const row of rows('money_scale_enforced')) {
    if (row.currency !== 'USD') continue;
    const create = () =>
      commerce.orders.create({ customerId: customer.id, items: [item(customer, Number(row.amount))] });
    if (row.must_reject) {
      assert.throws(create, /decimal places/, `${row.id}: ${row.amount} must be refused, not rounded`);
    } else {
      assert.equal(create().totalAmount, Number(row.amount), `${row.id}: ${row.amount} must be accepted`);
    }
  }
});
