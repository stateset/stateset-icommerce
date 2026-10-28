'use strict';
const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('finance decimal strings survive the native boundary without float rounding', async () => {
  const commerce = new Commerce(':memory:');
  try {
    const customer = await commerce.customers.create({ email: 'erp-decimals@example.invalid', firstName: 'Exact', lastName: 'Amounts' });
    const huge = '9007199254740993.01';
    const input = { customerId: customer.id, currency: 'USD', dueDate: '2026-12-01T00:00:00Z', items: [{ description: 'Exact', quantity: '1', unitPrice: huge }] };
    const invoice = await commerce.invoices.create(input);
    assert.equal(invoice.total, huge);
    assert.equal(invoice.totalExact, huge);
    assert.equal(invoice.dueDate.slice(0, 10), '2026-12-01');
    await assert.rejects(commerce.invoices.create({ ...input, currency: 'INVALID' }), /currency/i);
    assert.throws(() => commerce.invoices.create({ ...input, items: [{ description: 'Float', quantity: '1', unitPrice: 0.1 }] }), /String|string/);
    await commerce.invoices.send(invoice.id);
    const paid = await commerce.invoices.recordPayment(invoice.id, { amount: '0.01' });
    assert.equal(paid.amountPaid, '0.01');
    const credit = await commerce.credit.createCreditAccount({ customerId: customer.id, creditLimit: huge });
    assert.equal(credit.creditLimit, huge);
    assert.equal(typeof (await commerce.credit.checkCredit(customer.id, '0.01')).availableCredit, 'string');
    assert.throws(() => commerce.credit.checkCredit(customer.id, 0.1), /String|string/);
    const accounts = await commerce.generalLedger.initializeChartOfAccounts();
    const account = accounts.find(a => a.accountNumber === '1010');
    assert.equal(typeof await commerce.generalLedger.getAccountBalance(account.id), 'string');
  } finally { if (typeof commerce.close === 'function') await commerce.close(); }
});
