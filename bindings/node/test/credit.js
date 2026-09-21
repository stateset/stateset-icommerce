/**
 * Credit API tests for @stateset/embedded Node.js bindings.
 *
 * Limits and exposure are asserted through the `*Exact` twins. The binding
 * exposes the account lifecycle (create, adjust, suspend, reactivate) and the
 * credit check; holds and charges are engine-only, so exposure here is the
 * limit/available/used triple after each lifecycle step. Every test opens its
 * own `:memory:` store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

async function customer(commerce, email = 'buyer@example.com') {
  return commerce.customers.create({ email, firstName: 'Credit', lastName: 'Buyer' });
}

test('createCreditAccount opens an active account with the whole limit available; get, getByCustomer and list find it', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const account = await commerce.credit.createCreditAccount({
    customerId: buyer.id,
    creditLimit: 1000.5,
    paymentTerms: 'net_30',
    notes: 'Opened at onboarding',
  });
  assert.ok(account.id);
  assert.equal(account.customerId, buyer.id);
  assert.equal(account.status, 'Active');
  assert.equal(account.creditLimitExact, '1000.5');
  assert.equal(account.creditAvailableExact, '1000.5');
  assert.equal(account.creditUsedExact, '0');
  assert.equal(account.paymentTerms, 'net_30');

  assert.equal((await commerce.credit.getCreditAccount(account.id)).id, account.id);
  assert.equal((await commerce.credit.getCreditAccountByCustomer(buyer.id)).id, account.id);
  assert.deepEqual((await commerce.credit.listCreditAccounts()).map((a) => a.id), [account.id]);
  assert.equal(await commerce.credit.getCreditAccount('00000000-0000-0000-0000-000000000001'), null);
  assert.equal(await commerce.credit.getCreditAccountByCustomer('00000000-0000-0000-0000-000000000001'), null);
});

test('checkCredit without an account is declined and routed for approval', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const check = await commerce.credit.checkCredit(buyer.id, 10);
  assert.equal(check.approved, false);
  assert.equal(check.requiresApproval, true);
  assert.equal(check.availableCreditExact, '0');
  assert.match(check.reason, /no credit account/i);
});

test('checkCredit approves up to exactly the available credit and declines one cent past it', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  await commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 1000.5 });

  const atLimit = await commerce.credit.checkCredit(buyer.id, 1000.5);
  assert.equal(atLimit.approved, true);
  assert.equal(atLimit.requiresApproval, false);
  assert.equal(atLimit.reason, undefined);
  assert.equal(atLimit.availableCreditExact, '1000.5');

  const overLimit = await commerce.credit.checkCredit(buyer.id, 1000.51);
  assert.equal(overLimit.approved, false);
  assert.equal(overLimit.requiresApproval, true, 'an active account over its limit can still be approved by hand');
  assert.equal(overLimit.availableCreditExact, '1000.5');
  assert.match(overLimit.reason, /insufficient credit/i);
});

test('adjustCreditLimit moves the limit and the available credit together', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  await commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 1000.5 });

  const lowered = await commerce.credit.adjustCreditLimit(buyer.id, 250.25, 'quarterly risk review');
  assert.equal(lowered.creditLimitExact, '250.25');
  assert.equal(lowered.creditAvailableExact, '250.25');
  assert.equal(lowered.creditUsedExact, '0');
  assert.equal((await commerce.credit.checkCredit(buyer.id, 250.25)).approved, true);
  assert.equal((await commerce.credit.checkCredit(buyer.id, 250.26)).approved, false);

  const raised = await commerce.credit.adjustCreditLimit(buyer.id, 5000, 'good payment history');
  assert.equal(raised.creditLimitExact, '5000');
  assert.equal(raised.creditAvailableExact, '5000');
  assert.equal((await commerce.credit.getCreditAccountByCustomer(buyer.id)).creditLimitExact, '5000');
});

test('a suspended account declines every check without an approval path; reactivate restores it', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  await commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 500 });

  const suspended = await commerce.credit.suspendCreditAccount(buyer.id, 'two invoices past due');
  assert.equal(suspended.status, 'Suspended');
  assert.equal(suspended.creditAvailableExact, '500', 'suspension does not touch the numbers');

  const declined = await commerce.credit.checkCredit(buyer.id, 1);
  assert.equal(declined.approved, false);
  assert.equal(declined.requiresApproval, false, 'a suspended account is not a candidate for manual approval');
  assert.match(declined.reason, /suspended/i);
  assert.equal(declined.availableCreditExact, '500');

  const reactivated = await commerce.credit.reactivateCreditAccount(buyer.id);
  assert.equal(reactivated.status, 'Active');
  assert.equal((await commerce.credit.checkCredit(buyer.id, 1)).approved, true);
});

test('a customer has at most one credit account', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  await commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 100 });
  await assert.rejects(
    commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 200 }),
    (err) => err.code === 'CONFLICT',
  );
  assert.equal((await commerce.credit.getCreditAccountByCustomer(buyer.id)).creditLimitExact, '100', 'the first account is untouched');
});

test('lifecycle calls on a customer without an account are NOT_FOUND', async () => {
  const commerce = new Commerce(':memory:');
  const stranger = '00000000-0000-0000-0000-000000000001';
  await assert.rejects(commerce.credit.adjustCreditLimit(stranger, 5, 'x'), (err) => err.code === 'NOT_FOUND');
  await assert.rejects(commerce.credit.suspendCreditAccount(stranger, 'x'), (err) => err.code === 'NOT_FOUND');
  await assert.rejects(commerce.credit.reactivateCreditAccount(stranger), (err) => err.code === 'NOT_FOUND');
});

test('getOverLimitCustomers is empty while every account is within its limit', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  await commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 100 });
  await commerce.credit.adjustCreditLimit(buyer.id, 50, 'tightened');
  assert.deepEqual(await commerce.credit.getOverLimitCustomers(), []);
});

test('malformed inputs are refused with VALIDATION, never coerced', async () => {
  const commerce = new Commerce(':memory:');
  const buyer = await customer(commerce);
  const isValidation = (pattern) => (err) => err.code === 'VALIDATION' && pattern.test(err.message);
  await assert.rejects(commerce.credit.createCreditAccount({ customerId: 'not-a-uuid', creditLimit: 1 }), isValidation(/customer/i));
  await assert.rejects(commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: Number.NaN }), isValidation(/credit limit/i));
  await assert.rejects(commerce.credit.checkCredit('not-a-uuid', 1), isValidation(/uuid/i));
  await assert.rejects(commerce.credit.adjustCreditLimit('not-a-uuid', 1, 'x'), isValidation(/uuid/i));
  await assert.rejects(commerce.credit.suspendCreditAccount('not-a-uuid', 'x'), isValidation(/uuid/i));
  await assert.rejects(commerce.credit.getCreditAccount('not-a-uuid'), isValidation(/uuid/i));
  assert.deepEqual(await commerce.credit.listCreditAccounts(), [], 'nothing was written');
});

test(
  'a negative credit limit is refused on create',
  { todo: 'engine: createCreditAccount accepts creditLimit -5, opens an Active account with creditAvailableExact "-5" and lists it as over limit' },
  async () => {
    const commerce = new Commerce(':memory:');
    const buyer = await customer(commerce);
    await assert.rejects(
      commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: -5 }),
      (err) => err.code === 'VALIDATION' && /credit limit/i.test(err.message),
    );
    assert.equal(await commerce.credit.getCreditAccountByCustomer(buyer.id), null);
  },
);

test(
  'a negative credit limit is refused on adjust',
  { todo: 'engine: adjustCreditLimit accepts -1 and drives creditAvailableExact negative' },
  async () => {
    const commerce = new Commerce(':memory:');
    const buyer = await customer(commerce);
    await commerce.credit.createCreditAccount({ customerId: buyer.id, creditLimit: 100 });
    await assert.rejects(
      commerce.credit.adjustCreditLimit(buyer.id, -1, 'typo'),
      (err) => err.code === 'VALIDATION' && /limit/i.test(err.message),
    );
    assert.equal((await commerce.credit.getCreditAccountByCustomer(buyer.id)).creditLimitExact, '100');
  },
);
