/**
 * Month-end close orchestration tests for @stateset/embedded Node.js bindings.
 *
 * generalLedger.closeMonth(periodId, options) runs depreciation, revenue
 * recognition, FX revaluation, and the period close in order; `dryRun`
 * computes candidates without writing anything.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

async function setup(commerce) {
  const gl = commerce.generalLedger;
  await gl.initializeChartOfAccounts();

  // Wide open period covering today: GL auto-posting stamps entries with
  // today's date, which must fall inside an open period.
  const period = await gl.createPeriod({
    periodName: 'FY-wide',
    fiscalYear: 2026,
    periodNumber: 1,
    startDate: '2020-01-01',
    endDate: '2030-12-31',
  });
  assert.equal(period.status, 'future');
  const opened = await gl.openPeriod(period.id);
  assert.equal(opened.status, 'open');

  // Asset: $1200 over 12 months straight-line; all periods due by period end.
  const asset = await commerce.fixedAssets.create({
    name: 'Espresso machine',
    category: 'machinery',
    acquisitionDate: '2026-01-01',
    acquisitionCost: '1200.00',
    salvageValue: '0',
    usefulLifeMonths: 12,
    depreciationMethod: 'straight_line',
  });
  await commerce.fixedAssets.placeInService(asset.id, '2026-01-01');
  await commerce.fixedAssets.generateSchedule(asset.id);
  return { period, asset };
}

test('GeneralLedger.closeMonth', async (t) => {
  await t.test('dry run reports candidates without writing', async () => {
    const commerce = new Commerce(':memory:');
    const { period, asset } = await setup(commerce);

    const report = await commerce.generalLedger.closeMonth(period.id, { dryRun: true });
    assert.equal(report.dryRun, true);
    assert.equal(report.periodId, period.id);
    assert.equal(report.periodName, 'FY-wide');
    assert.equal(report.depreciation.status, 'dry_run');
    assert.equal(report.depreciation.entryCount, 12);
    assert.equal(report.depreciation.totalAmount, '1200.00');
    assert.equal(report.revenueRecognition.status, 'dry_run');
    assert.equal(report.fxRevaluation.status, 'skipped');
    assert.equal(report.periodClose.status, 'dry_run');
    assert.ok(report.closingEntry === null || report.closingEntry === undefined);
    assert.equal(report.periodStatus, 'open');

    // Nothing was written.
    const after = await commerce.fixedAssets.get(asset.id);
    assert.equal(after.accumulatedDepreciation, '0');
  });

  await t.test('skip flags mark steps skipped', async () => {
    const commerce = new Commerce(':memory:');
    const { period } = await setup(commerce);

    const report = await commerce.generalLedger.closeMonth(period.id, {
      skipDepreciation: true,
      skipRevenueRecognition: true,
      skipFxRevaluation: true,
      skipPeriodClose: true,
    });
    assert.equal(report.depreciation.status, 'skipped');
    assert.equal(report.revenueRecognition.status, 'skipped');
    assert.equal(report.fxRevaluation.status, 'skipped');
    assert.equal(report.periodClose.status, 'skipped');
    assert.equal(report.periodStatus, 'open');
  });

  await t.test('real run posts depreciation (period close skipped)', async () => {
    const commerce = new Commerce(':memory:');
    const { period, asset } = await setup(commerce);

    // Without GL auto-posting there is no P&L activity, so skip the final
    // period close (which requires net income) and exercise the wet path
    // for the depreciation step.
    const report = await commerce.generalLedger.closeMonth(period.id, {
      closedBy: 'binding-test',
      skipPeriodClose: true,
    });
    assert.equal(report.dryRun, false);
    assert.equal(report.depreciation.status, 'executed');
    assert.equal(report.depreciation.entryCount, 12);
    assert.equal(report.depreciation.totalAmount, '1200.00');
    assert.deepEqual(report.depreciation.warnings, []);
    assert.equal(report.fxRevaluation.status, 'skipped');
    assert.equal(report.periodClose.status, 'skipped');
    assert.equal(report.periodStatus, 'open');

    const after = await commerce.fixedAssets.get(asset.id);
    assert.equal(after.accumulatedDepreciation, '1200.00');
    assert.equal(after.status, 'fully_depreciated');
  });

  await t.test('rejects an invalid period id', async () => {
    const commerce = new Commerce(':memory:');
    await assert.rejects(
      () => commerce.generalLedger.closeMonth('not-a-uuid'),
      /Invalid UUID/,
    );
  });
});
