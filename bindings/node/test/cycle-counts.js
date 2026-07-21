/**
 * Cycle Counts API tests for @stateset/embedded Node.js bindings.
 *
 * Quantities are exchanged as exact decimal strings; enums as snake_case
 * strings; timestamps as RFC 3339 strings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('CycleCounts: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('cycleCounts API exists', () => {
    assert.ok(commerce.cycleCounts, 'cycleCounts API should exist');
  });

  const warehouse = await commerce.warehouse.createWarehouse({
    code: 'WH-CC',
    name: 'Cycle Count Warehouse',
  });
  const location = await commerce.warehouse.createLocation({
    warehouseId: warehouse.id,
    locationType: 'bin',
    zone: 'A',
  });

  let count;
  await t.test('create returns a draft count with expected lines', async () => {
    count = await commerce.cycleCounts.create({
      warehouseId: warehouse.id,
      locationId: location.id,
      countedBy: 'counter@example.com',
      lines: [
        { sku: 'CC-SKU-1', expectedQuantity: '100' },
        { sku: 'CC-SKU-2', expectedQuantity: '25.5' },
      ],
    });
    assert.ok(count.id);
    assert.equal(count.status, 'draft');
    assert.equal(count.warehouseId, warehouse.id);
    assert.equal(count.countedBy, 'counter@example.com');
    assert.equal(count.lines.length, 2);
    assert.equal(count.lines[0].expectedQuantity, '100');
    assert.equal(count.lines[1].expectedQuantity, '25.5');
    assert.ok(count.lines[0].countedQuantity == null);
  });

  await t.test('get and list find the count', async () => {
    const found = await commerce.cycleCounts.get(count.id);
    assert.ok(found);
    assert.equal(found.id, count.id);
    const listed = await commerce.cycleCounts.list({
      warehouseId: warehouse.id,
      status: 'draft',
    });
    assert.ok(listed.some((c) => c.id === count.id));
  });

  await t.test('start transitions draft -> in_progress', async () => {
    const started = await commerce.cycleCounts.start(count.id);
    assert.equal(started.status, 'in_progress');
  });

  await t.test('recordCounts records physical counts with variances', async () => {
    const recorded = await commerce.cycleCounts.recordCounts(count.id, [
      { sku: 'CC-SKU-1', countedQuantity: '103' },
      { sku: 'CC-SKU-2', countedQuantity: '25.5' },
    ]);
    const line1 = recorded.lines.find((l) => l.sku === 'CC-SKU-1');
    const line2 = recorded.lines.find((l) => l.sku === 'CC-SKU-2');
    assert.equal(line1.countedQuantity, '103');
    assert.equal(line1.variance, '3');
    assert.equal(line2.variance, '0.0');
  });

  await t.test('complete applies variances and finishes the count', async () => {
    const completed = await commerce.cycleCounts.complete(count.id);
    assert.equal(completed.status, 'completed');
    assert.ok(completed.completedAt);
  });

  await t.test('cancel abandons a draft count', async () => {
    const other = await commerce.cycleCounts.create({
      warehouseId: warehouse.id,
      lines: [{ sku: 'CC-SKU-3', expectedQuantity: '10' }],
    });
    const cancelled = await commerce.cycleCounts.cancel(other.id);
    assert.equal(cancelled.status, 'cancelled');
  });
});
