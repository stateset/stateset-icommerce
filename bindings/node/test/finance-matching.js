/**
 * Finance hardening tests: accountsPayable.threeWayMatch and
 * generalLedger.revalue for @stateset/embedded Node.js bindings.
 *
 * Money crosses as exact decimal strings; dates as ISO strings; enums as
 * snake_case strings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('AccountsPayable.threeWayMatch', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('bill without a purchase order is not_required', async () => {
    const bill = await commerce.accountsPayable.createBill({
      supplierId: '3f2504e0-4f89-41d3-9a0c-0305e82c3301',
      dueDate: new Date('2026-08-01T00:00:00Z').toISOString(),
    });
    const result = await commerce.accountsPayable.threeWayMatch(bill.id, '5');
    assert.equal(result.matchStatus, 'not_required');
    assert.equal(result.lines.length, 0);
  });

  await t.test('rejects an invalid bill UUID', async () => {
    await assert.rejects(() => commerce.accountsPayable.threeWayMatch('not-a-uuid'), /Invalid UUID/);
  });

  await t.test('rejects an invalid tolerance decimal', async () => {
    await assert.rejects(
      () =>
        commerce.accountsPayable.threeWayMatch('3f2504e0-4f89-41d3-9a0c-0305e82c3301', 'abc'),
      /Invalid tolerance_percent decimal/,
    );
  });
});

test('GeneralLedger.revalue', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('revalues with no foreign-currency accounts as a no-op', async () => {
    await commerce.generalLedger.initializeChartOfAccounts();
    const result = await commerce.generalLedger.revalue('2026-07-01', 'USD');
    assert.equal(result.asOfDate, '2026-07-01');
    assert.equal(result.baseCurrency, 'USD');
    assert.equal(result.totalUnrealizedGainLoss, '0');
    assert.equal(result.lines.length, 0);
    assert.ok(result.journalEntry == null);
  });

  await t.test('rejects an invalid date', async () => {
    await assert.rejects(() => commerce.generalLedger.revalue('July 1'), /Invalid date format/);
  });

  await t.test('rejects an invalid base currency', async () => {
    await assert.rejects(
      () => commerce.generalLedger.revalue('2026-07-01', 'NOPE'),
      /Invalid base currency code/,
    );
  });
});
