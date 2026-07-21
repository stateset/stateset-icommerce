/**
 * Fixed Assets API tests for @stateset/embedded Node.js bindings.
 *
 * Money is exchanged as exact decimal strings (no f64 precision loss);
 * dates cross as ISO strings; enums as snake_case strings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('FixedAssets: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('fixedAssets API exists and is supported', async () => {
    assert.ok(commerce.fixedAssets, 'fixedAssets API should exist');
    assert.equal(await commerce.fixedAssets.isSupported(), true);
  });

  let asset;
  await t.test('create returns a draft asset with exact-string money', async () => {
    asset = await commerce.fixedAssets.create({
      name: 'Forklift',
      category: 'machinery',
      acquisitionDate: '2026-01-01',
      acquisitionCost: '10000.00',
      salvageValue: '1000.00',
      usefulLifeMonths: 36,
      depreciationMethod: 'straight_line',
    });
    assert.ok(asset.id);
    assert.ok(asset.assetNumber.startsWith('FA-'));
    assert.equal(asset.status, 'draft');
    assert.equal(asset.category, 'machinery');
    assert.equal(asset.acquisitionCost, '10000.00');
    assert.equal(asset.salvageValue, '1000.00');
    assert.equal(asset.accumulatedDepreciation, '0');
    assert.equal(asset.bookValue, '10000.00');
    assert.equal(asset.depreciationMethod, 'straight_line');
  });

  await t.test('get and list find the asset', async () => {
    const found = await commerce.fixedAssets.get(asset.id);
    assert.ok(found);
    assert.equal(found.id, asset.id);
    const listed = await commerce.fixedAssets.list({ category: 'machinery' });
    assert.ok(listed.some((a) => a.id === asset.id));
  });

  await t.test('update changes the name', async () => {
    const updated = await commerce.fixedAssets.update(asset.id, { name: 'Forklift A' });
    assert.equal(updated.name, 'Forklift A');
  });

  await t.test('placeInService transitions draft -> in_service', async () => {
    const inService = await commerce.fixedAssets.placeInService(asset.id, '2026-02-01');
    assert.equal(inService.status, 'in_service');
    assert.equal(inService.inServiceDate, '2026-02-01');
  });

  await t.test('generateSchedule produces a straight-line schedule summing to the base', async () => {
    const schedule = await commerce.fixedAssets.generateSchedule(asset.id);
    assert.equal(schedule.assetId, asset.id);
    assert.equal(schedule.method, 'straight_line');
    assert.equal(schedule.entries.length, 36);
    assert.equal(schedule.totalDepreciation, '9000.00');
    assert.equal(schedule.entries[0].amount, '250.00');
    assert.equal(schedule.entries[0].status, 'scheduled');

    const persisted = await commerce.fixedAssets.getSchedule(asset.id);
    assert.ok(persisted);
    assert.equal(persisted.entries.length, 36);
  });

  await t.test('postDepreciation posts periods and grows accumulated depreciation', async () => {
    const after = await commerce.fixedAssets.postDepreciation(asset.id, 2);
    assert.equal(after.accumulatedDepreciation, '500.00');
    assert.equal(after.bookValue, '9500.00');

    const schedule = await commerce.fixedAssets.getSchedule(asset.id);
    assert.equal(schedule.entries[0].status, 'posted');
    assert.equal(schedule.entries[1].status, 'posted');
    assert.equal(schedule.entries[2].status, 'scheduled');
  });

  await t.test('dispose records proceeds and gain/loss', async () => {
    const disposed = await commerce.fixedAssets.dispose(asset.id, '9800.00', '2026-06-30', 'sold');
    assert.equal(disposed.status, 'disposed');
    assert.ok(disposed.disposal);
    assert.equal(disposed.disposal.proceeds, '9800.00');
    assert.equal(disposed.disposal.bookValueAtDisposal, '9500.00');
    assert.equal(disposed.disposal.gainLoss, '300.00');
    assert.equal(disposed.disposal.disposalDate, '2026-06-30');
    assert.equal(disposed.disposal.notes, 'sold');
  });

  await t.test('writeOff disposes another asset with zero proceeds', async () => {
    let other = await commerce.fixedAssets.create({
      name: 'Old laptop',
      category: 'computer_hardware',
      acquisitionDate: '2025-01-01',
      acquisitionCost: '1200.00',
      salvageValue: '0',
      usefulLifeMonths: 24,
      depreciationMethod: 'straight_line',
    });
    other = await commerce.fixedAssets.placeInService(other.id, '2025-01-01');
    const written = await commerce.fixedAssets.writeOff(other.id, '2026-01-01', 'damaged');
    assert.equal(written.status, 'written_off');
    assert.equal(written.disposal.proceeds, '0');
  });
});
