/**
 * Accounts receivable tests for @stateset/embedded Node.js bindings.
 *
 * Receivables are the open balances on invoices. An invoice starts in draft
 * with its full total due (30 days out); `invoices.recordPayment` reduces
 * `balance_due` and the AR views (aging summary, total outstanding, DSO) are
 * computed from what is still owed. Credit memos are customer-level credits
 * that stay "unapplied" until used; an open memo can be voided once.
 */

'use strict';

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const UNKNOWN_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';
const ZERO_AGING = {
  currentExact: '0',
  days130Exact: '0',
  days3160Exact: '0',
  days6190Exact: '0',
  daysOver90Exact: '0',
  totalExact: '0',
};

async function customer(commerce, email = 'ar@example.com') {
  return commerce.customers.create({ email, firstName: 'A', lastName: 'R' });
}

async function invoice(commerce, customerId, unitPriceExact, quantity = 1) {
  return commerce.invoices.create({
    customerId,
    items: [{ description: 'Consulting', quantity, unitPriceExact }],
  });
}

function exactAging(summary) {
  return Object.fromEntries(Object.keys(ZERO_AGING).map((k) => [k, summary[k]]));
}

test('an empty ledger reports zero aging, zero outstanding, zero DSO', async () => {
  const commerce = new Commerce(':memory:');
  const aging = await commerce.accountsReceivable.getAgingSummary();
  assert.deepEqual(exactAging(aging), ZERO_AGING);
  assert.equal(aging.total, 0);
  assert.equal(await commerce.accountsReceivable.getTotalOutstanding(), 0);
  assert.equal(await commerce.accountsReceivable.getDso(30), 0);
});

test('invoice -> partial payment -> outstanding balance -> aging report', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const inv = await invoice(commerce, cust.id, '50.00', 2);
  assert.equal(inv.totalExact, '100.00');
  assert.equal(inv.amountPaidExact, '0');
  assert.equal(inv.status, 'draft');
  // Due 30 days out, so the whole balance ages as "current".
  assert.ok(Date.parse(inv.dueDate) > Date.now());

  let aging = await commerce.accountsReceivable.getAgingSummary();
  assert.deepEqual(exactAging(aging), { ...ZERO_AGING, currentExact: '100.00', totalExact: '100.00' });
  assert.equal(await commerce.accountsReceivable.getTotalOutstanding(), 100);

  const paid = await commerce.invoices.recordPayment(inv.id, {
    amountExact: '40.00',
    paymentMethod: 'check',
    reference: 'CHK-1001',
  });
  assert.equal(paid.status, 'partially_paid');
  assert.equal(paid.amountPaidExact, '40.00');
  assert.equal(paid.totalExact, '100.00');

  aging = await commerce.accountsReceivable.getAgingSummary();
  assert.deepEqual(exactAging(aging), { ...ZERO_AGING, currentExact: '60.00', totalExact: '60.00' });
  assert.equal(aging.current, 60);
  assert.equal(aging.total, 60);
  assert.equal(await commerce.accountsReceivable.getTotalOutstanding(), 60);

  // DSO = outstanding / sales in window x days = 60 / 100 x 30 = 18.
  assert.equal(await commerce.accountsReceivable.getDso(30), 18);
  assert.equal(await commerce.accountsReceivable.getDso(90), 54);

  const settled = await commerce.invoices.recordPayment(inv.id, { amountExact: '60.00' });
  assert.equal(settled.status, 'paid');
  assert.equal(settled.amountPaidExact, '100.00');
  assert.deepEqual(exactAging(await commerce.accountsReceivable.getAgingSummary()), ZERO_AGING);
  assert.equal(await commerce.accountsReceivable.getTotalOutstanding(), 0);
  assert.equal(await commerce.accountsReceivable.getDso(30), 0);
});

test('aging sums exact balances across customers and drops voided invoices', async () => {
  const commerce = new Commerce(':memory:');
  const a = await customer(commerce, 'a@example.com');
  const b = await customer(commerce, 'b@example.com');
  const ia = await invoice(commerce, a.id, '19.99', 3); // 59.97
  const ib = await invoice(commerce, b.id, '0.01', 7); // 0.07
  const ic = await invoice(commerce, b.id, '250.00'); // 250.00, voided below

  let aging = await commerce.accountsReceivable.getAgingSummary();
  assert.equal(aging.currentExact, '310.04');
  assert.equal(aging.totalExact, '310.04');

  const voided = await commerce.invoices.void(ic.id);
  assert.equal(voided.status, 'voided');
  await commerce.invoices.recordPayment(ia.id, { amountExact: '9.97' });

  aging = await commerce.accountsReceivable.getAgingSummary();
  assert.equal(aging.currentExact, '50.07'); // 50.00 + 0.07
  assert.equal(aging.totalExact, '50.07');
  assert.equal(await commerce.accountsReceivable.getTotalOutstanding(), 50.07);
  assert.equal(ib.totalExact, '0.07');
});

test('overpaying an invoice is refused with VALIDATION and leaves the receivable intact', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const inv = await invoice(commerce, cust.id, '25.00');
  await assert.rejects(
    commerce.invoices.recordPayment(inv.id, { amountExact: '25.01' }),
    (err) => err.code === 'VALIDATION' && /exceeds/.test(err.message),
  );
  assert.equal((await commerce.accountsReceivable.getAgingSummary()).totalExact, '25.00');
});

test('createCreditMemo -> getCreditMemo -> listCreditMemos -> getUnappliedCredits', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const inv = await invoice(commerce, cust.id, '80.00');

  const memo = await commerce.accountsReceivable.createCreditMemo({
    customerId: cust.id,
    originalInvoiceId: inv.id,
    reason: 'returned_goods',
    amount: 12.34,
    notes: 'one unit returned',
  });
  assert.ok(memo.id);
  assert.match(memo.creditMemoNumber, /^CM-/);
  assert.equal(memo.customerId, cust.id);
  assert.equal(memo.amountExact, '12.34');
  assert.equal(memo.amount, 12.34);
  assert.equal(memo.status, 'Open');
  assert.equal(memo.reason, 'ReturnedGoods');
  assert.ok(!Number.isNaN(Date.parse(memo.createdAt)));

  assert.deepEqual(await commerce.accountsReceivable.getCreditMemo(memo.id), memo);
  assert.equal(await commerce.accountsReceivable.getCreditMemo(UNKNOWN_ID), null);

  const listed = await commerce.accountsReceivable.listCreditMemos();
  assert.equal(listed.length, 1);
  assert.equal(listed[0].id, memo.id);

  const unapplied = await commerce.accountsReceivable.getUnappliedCredits(cust.id);
  assert.equal(unapplied.length, 1);
  assert.equal(unapplied[0].amountExact, '12.34');
  assert.deepEqual(await commerce.accountsReceivable.getUnappliedCredits(UNKNOWN_ID), []);

  // A credit memo is not a payment: the invoice still owes its full total.
  assert.equal((await commerce.accountsReceivable.getAgingSummary()).totalExact, '80.00');
});

test('credit memo reasons map onto the engine enum, unknown is refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const cases = [
    ['returned_goods', 'ReturnedGoods'],
    ['pricing_error', 'PricingError'],
    ['billing_error', 'PricingError'],
    ['overpayment', 'Overpayment'],
    ['damaged', 'Damaged'],
    ['service_credit', 'ServiceCredit'],
    ['goodwill', 'GoodwillAdjustment'],
    ['other', 'Other'],
  ];
  for (const [reason, expected] of cases) {
    const memo = await commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason, amount: 1 });
    assert.equal(memo.reason, expected, `reason '${reason}'`);
  }
  assert.equal((await commerce.accountsReceivable.listCreditMemos()).length, cases.length);
  await assert.rejects(
    commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason: 'because', amount: 1 }),
    (err) => err.code === 'VALIDATION' && /credit memo reason 'because'/.test(err.message),
  );
  assert.equal((await commerce.accountsReceivable.listCreditMemos()).length, cases.length);
});

test('voidCreditMemo flips open -> voided once; a second void is CONFLICT', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const memo = await commerce.accountsReceivable.createCreditMemo({
    customerId: cust.id,
    reason: 'goodwill',
    amount: 5,
  });

  const voided = await commerce.accountsReceivable.voidCreditMemo(memo.id);
  assert.equal(voided.id, memo.id);
  assert.equal(voided.status, 'Voided');
  assert.equal(voided.amountExact, '5');
  assert.equal((await commerce.accountsReceivable.getCreditMemo(memo.id)).status, 'Voided');

  await assert.rejects(
    commerce.accountsReceivable.voidCreditMemo(memo.id),
    (err) => err.code === 'CONFLICT' && /already voided/.test(err.message),
  );
  await assert.rejects(
    commerce.accountsReceivable.voidCreditMemo(UNKNOWN_ID),
    (err) => err.code === 'NOT_FOUND',
  );
});

test(
  'a voided credit memo is no longer an unapplied credit',
  async () => {
    const commerce = new Commerce(':memory:');
    const cust = await customer(commerce);
    const memo = await commerce.accountsReceivable.createCreditMemo({
      customerId: cust.id,
      reason: 'goodwill',
      amount: 5,
    });
    await commerce.accountsReceivable.voidCreditMemo(memo.id);
    assert.deepEqual(await commerce.accountsReceivable.getUnappliedCredits(cust.id), []);
  },
);

test(
  'createCreditMemo refuses a customer the store does not have',
  async () => {
    const commerce = new Commerce(':memory:');
    await assert.rejects(
      commerce.accountsReceivable.createCreditMemo({ customerId: UNKNOWN_ID, reason: 'goodwill', amount: 1 }),
      (err) => err.code === 'NOT_FOUND' || err.code === 'VALIDATION',
    );
    assert.deepEqual(await commerce.accountsReceivable.listCreditMemos(), []);
  },
);

test('createCreditMemo refuses a non-positive amount with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  for (const amount of [0, -1]) {
    await assert.rejects(
      commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason: 'other', amount }),
      (err) => err.code === 'VALIDATION' && /greater than zero/.test(err.message),
      `amount ${amount}`,
    );
  }
  assert.deepEqual(await commerce.accountsReceivable.listCreditMemos(), []);
});

test('malformed UUIDs and a NaN amount are refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);

  await assert.rejects(
    commerce.accountsReceivable.createCreditMemo({ customerId: 'not-a-uuid', reason: 'other', amount: 1 }),
    (err) => err.code === 'VALIDATION' && /customer UUID/i.test(err.message),
  );
  await assert.rejects(
    commerce.accountsReceivable.createCreditMemo({
      customerId: cust.id,
      originalInvoiceId: 'not-a-uuid',
      reason: 'other',
      amount: 1,
    }),
    (err) => err.code === 'VALIDATION' && /invoice UUID 'not-a-uuid'/i.test(err.message),
  );
  await assert.rejects(
    commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason: 'other', amount: NaN }),
    (err) => err.code === 'VALIDATION' && /amount/i.test(err.message),
  );
  for (const op of ['getCreditMemo', 'voidCreditMemo', 'getUnappliedCredits']) {
    await assert.rejects(
      commerce.accountsReceivable[op]('nope'),
      (err) => err.code === 'VALIDATION' && /Invalid UUID/.test(err.message),
      `${op} should reject a malformed UUID`,
    );
  }
  assert.deepEqual(await commerce.accountsReceivable.listCreditMemos(), []);
});
