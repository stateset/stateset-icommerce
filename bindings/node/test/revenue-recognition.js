/**
 * Revenue Recognition (ASC 606) API tests for @stateset/embedded Node.js bindings.
 *
 * Money is exchanged as exact decimal strings (no f64 precision loss);
 * dates cross as ISO strings; enums as snake_case strings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('RevenueRecognition: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('revenueRecognition API exists and is supported', async () => {
    assert.ok(commerce.revenueRecognition, 'revenueRecognition API should exist');
    assert.equal(await commerce.revenueRecognition.isSupported(), true);
  });

  const customer = await commerce.customers.create({
    email: 'rev@example.com',
    firstName: 'Rev',
    lastName: 'Customer',
  });

  let contract;
  await t.test('createContract allocates obligations exactly', async () => {
    contract = await commerce.revenueRecognition.createContract({
      customerId: customer.id,
      transactionPrice: '1200.00',
      effectiveDate: '2026-01-01',
      obligations: [
        {
          description: 'Annual support',
          allocatedAmount: '1200.00',
          recognitionMethod: 'ratable_over_time',
          recognitionStart: '2026-01-01',
          recognitionEnd: '2026-12-31',
        },
      ],
    });
    assert.ok(contract.id);
    assert.ok(contract.contractNumber.startsWith('RC-'));
    assert.equal(contract.transactionPrice, '1200.00');
    assert.equal(contract.obligations.length, 1);
    assert.equal(contract.obligations[0].allocatedAmount, '1200.00');
    assert.equal(contract.obligations[0].recognitionMethod, 'ratable_over_time');
    assert.equal(contract.totalRecognized, '0');
    assert.equal(contract.deferredBalance, '1200.00');
  });

  await t.test('getContract and listContracts find the contract', async () => {
    const found = await commerce.revenueRecognition.getContract(contract.id);
    assert.ok(found);
    assert.equal(found.id, contract.id);
    const listed = await commerce.revenueRecognition.listContracts({
      customerId: customer.id,
    });
    assert.ok(listed.some((c) => c.id === contract.id));
  });

  await t.test('updateContract activates the contract', async () => {
    const updated = await commerce.revenueRecognition.updateContract(contract.id, {
      status: 'active',
    });
    assert.equal(updated.status, 'active');
  });

  let obligation;
  await t.test('listObligations returns the obligation', async () => {
    const obligations = await commerce.revenueRecognition.listObligations(contract.id);
    assert.equal(obligations.length, 1);
    obligation = obligations[0];
    assert.equal(obligation.contractId, contract.id);
    assert.equal(obligation.deferredAmount, '1200.00');
  });

  await t.test('generateSchedule builds a 12-month ratable schedule', async () => {
    const schedule = await commerce.revenueRecognition.generateSchedule(obligation.id);
    assert.equal(schedule.obligationId, obligation.id);
    assert.equal(schedule.method, 'ratable_over_time');
    assert.equal(schedule.entries.length, 12);
    assert.equal(schedule.totalAmount, '1200.00');
    assert.equal(schedule.entries[0].amount, '100.00');
    assert.equal(schedule.entries[0].status, 'deferred');
    assert.equal(schedule.entries[0].periodStart, '2026-01-01');
    assert.equal(schedule.deferredTotal, '1200.00');

    const persisted = await commerce.revenueRecognition.getSchedule(obligation.id);
    assert.ok(persisted);
    assert.equal(persisted.entries.length, 12);
  });

  await t.test('recognize recognizes periods through a date', async () => {
    const schedule = await commerce.revenueRecognition.recognize(obligation.id, '2026-03-15');
    assert.equal(schedule.recognizedTotal, '300.00');
    assert.equal(schedule.deferredTotal, '900.00');
    assert.equal(schedule.entries[0].status, 'recognized');
    assert.equal(schedule.entries[2].status, 'recognized');
    assert.equal(schedule.entries[3].status, 'deferred');

    const after = await commerce.revenueRecognition.getContract(contract.id);
    assert.equal(after.totalRecognized, '300.00');
    assert.equal(after.deferredBalance, '900.00');
  });
});
