/**
 * Cost accounting tests for @stateset/embedded Node.js bindings.
 *
 * An item cost record carries a standard cost, its material/labor/overhead
 * roll-up, and the moving average and last purchase costs. Inputs cross as
 * floats (the legacy shape) but every output has an exact decimal `*Exact`
 * twin, and the tests assert those. `updateAverageCost` performs a weighted
 * average against the quantity currently on hand in inventory.
 */

'use strict';

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

/** Sum exact decimal strings without floats: scale to the widest fraction and add as BigInt. */
function addExact(...values) {
  const scale = Math.max(...values.map((v) => (v.split('.')[1] || '').length));
  const total = values.reduce((acc, v) => {
    const [whole, frac = ''] = v.split('.');
    const negative = whole.startsWith('-');
    const digits = BigInt(whole.replace('-', '') + frac.padEnd(scale, '0'));
    return acc + (negative ? -digits : digits);
  }, 0n);
  if (scale === 0) return total.toString();
  const abs = (total < 0n ? -total : total).toString().padStart(scale + 1, '0');
  return `${total < 0n ? '-' : ''}${abs.slice(0, -scale)}.${abs.slice(-scale)}`;
}

/** Drop trailing fractional zeros: a computed average may carry the scale of its operands. */
function trimScale(value) {
  return value.includes('.') ? value.replace(/0+$/, '').replace(/\.$/, '') : value;
}

test('exact-decimal helpers', () => {
  assert.equal(addExact('6.25', '2.50', '1.25'), '10.00');
  assert.equal(addExact('0.1', '0.2'), '0.3');
  assert.equal(addExact('10', '-1.5'), '8.5');
  assert.equal(trimScale('11.250'), '11.25');
  assert.equal(trimScale('10.00'), '10');
  assert.equal(trimScale('7'), '7');
});

test('setItemCost -> getItemCost -> listItemCosts round-trips an exact cost record', async () => {
  const commerce = new Commerce(':memory:');
  const cost = await commerce.costAccounting.setItemCost({
    sku: 'WIDGET-1',
    costMethod: 'standard',
    standardCost: 10,
    materialCost: 6.25,
    laborCost: 2.5,
    overheadCost: 1.25,
  });
  assert.ok(cost.id);
  assert.equal(cost.sku, 'WIDGET-1');
  assert.equal(cost.costMethod, 'Standard');
  assert.equal(cost.standardCostExact, '10');
  assert.equal(cost.materialCostExact, '6.25');
  assert.equal(cost.laborCostExact, '2.5');
  assert.equal(cost.overheadCostExact, '1.25');
  // A fresh record seeds average and last cost from the standard cost.
  assert.equal(cost.averageCostExact, '10');
  assert.equal(cost.lastCostExact, '10');
  // Float twins agree with the exact strings.
  assert.equal(cost.standardCost, 10);
  assert.equal(cost.materialCost, 6.25);

  assert.deepEqual(await commerce.costAccounting.getItemCost('WIDGET-1'), cost);
  assert.equal(await commerce.costAccounting.getItemCost('NOPE'), null);

  const listed = await commerce.costAccounting.listItemCosts();
  assert.equal(listed.length, 1);
  assert.equal(listed[0].sku, 'WIDGET-1');
});

test('cost roll-up: material + labor + overhead equals the standard cost, exactly', async () => {
  const commerce = new Commerce(':memory:');
  const cost = await commerce.costAccounting.setItemCost({
    sku: 'ASSY-1',
    costMethod: 'standard',
    standardCost: 10,
    materialCost: 6.25,
    laborCost: 2.5,
    overheadCost: 1.25,
  });
  const rollup = addExact(cost.materialCostExact, cost.laborCostExact, cost.overheadCostExact);
  assert.equal(rollup, '10.00');
  assert.equal(addExact(rollup, '0'), addExact(cost.standardCostExact, '0.00'));
});

test('setItemCost on an existing SKU updates only the fields sent', async () => {
  const commerce = new Commerce(':memory:');
  await commerce.costAccounting.setItemCost({
    sku: 'WIDGET-1',
    costMethod: 'standard',
    standardCost: 10,
    materialCost: 6.25,
    laborCost: 2.5,
    overheadCost: 1.25,
  });
  const updated = await commerce.costAccounting.setItemCost({ sku: 'WIDGET-1', laborCost: 3 });
  assert.equal(updated.laborCostExact, '3');
  assert.equal(updated.materialCostExact, '6.25');
  assert.equal(updated.overheadCostExact, '1.25');
  assert.equal(updated.standardCostExact, '10');
  assert.equal(updated.costMethod, 'Standard');
  assert.equal((await commerce.costAccounting.listItemCosts()).length, 1);

  const method = await commerce.costAccounting.setItemCost({ sku: 'WIDGET-1', costMethod: 'fifo' });
  assert.equal(method.costMethod, 'Fifo');
  assert.equal(method.laborCostExact, '3');
});

test('cost method strings map onto the engine enum, absent is Average, unknown is refused', async () => {
  const commerce = new Commerce(':memory:');
  const cases = [
    ['standard', 'Standard'],
    ['average', 'Average'],
    ['fifo', 'Fifo'],
    ['lifo', 'Lifo'],
    ['LIFO', 'Lifo'],
    [undefined, 'Average'],
  ];
  for (const [input, expected] of cases) {
    const cost = await commerce.costAccounting.setItemCost({
      sku: `M-${String(input)}`,
      costMethod: input,
      standardCost: 1,
    });
    assert.equal(cost.costMethod, expected, `costMethod '${input}'`);
  }
  await assert.rejects(
    commerce.costAccounting.setItemCost({ sku: 'M-bogus', costMethod: 'bogus', standardCost: 1 }),
    (err) => err.code === 'VALIDATION' && /cost method 'bogus'/.test(err.message),
  );
  assert.equal(await commerce.costAccounting.getItemCost('M-bogus'), null);
});

test('updateAverageCost computes the weighted average against on-hand stock and a purchase variance', async () => {
  const commerce = new Commerce(':memory:');
  await commerce.inventory.createItem({ sku: 'AVG-1', name: 'Averaged', initialQuantity: 10 });
  await commerce.costAccounting.setItemCost({ sku: 'AVG-1', costMethod: 'average', standardCost: 10 });

  // 10 on hand @ 10.00, receive 10 @ 12.50: (100 + 125) / 20 = 11.25.
  // The engine's Decimal keeps the operands' scale ("11.250"); the value is exact.
  const after = await commerce.costAccounting.updateAverageCost('AVG-1', 10, 12.5);
  assert.equal(trimScale(after.averageCostExact), '11.25');
  assert.equal(after.lastCostExact, '12.5');
  assert.equal(after.standardCostExact, '10');
  assert.equal(after.averageCost, 11.25);

  // Purchase price variance = average - standard = 1.25 per unit.
  assert.equal(trimScale(addExact(after.averageCostExact, '-' + after.standardCostExact)), '1.25');

  // A second receipt at the running average leaves it unchanged.
  const again = await commerce.costAccounting.updateAverageCost('AVG-1', 5, 11.25);
  assert.equal(trimScale(again.averageCostExact), '11.25');
  assert.equal(again.lastCostExact, '11.25');
});

test('updateAverageCost on a SKU with no cost record seeds one from the receipt', async () => {
  const commerce = new Commerce(':memory:');
  assert.equal(await commerce.costAccounting.getItemCost('NEW-1'), null);
  const cost = await commerce.costAccounting.updateAverageCost('NEW-1', 4, 3.33);
  assert.equal(cost.sku, 'NEW-1');
  assert.equal(cost.costMethod, 'Average');
  assert.equal(cost.standardCostExact, '3.33');
  assert.equal(cost.averageCostExact, '3.33');
  assert.equal(cost.lastCostExact, '3.33');
  assert.equal(cost.materialCostExact, '0');
  assert.ok(await commerce.costAccounting.getItemCost('NEW-1'));
});

test('getTotalInventoryValue is on-hand quantity times average cost across tracked SKUs', async () => {
  const commerce = new Commerce(':memory:');
  assert.equal(await commerce.costAccounting.getTotalInventoryValue(), 0);

  await commerce.inventory.createItem({ sku: 'VAL-1', name: 'One', initialQuantity: 10 });
  await commerce.inventory.createItem({ sku: 'VAL-2', name: 'Two', initialQuantity: 4 });
  await commerce.costAccounting.setItemCost({ sku: 'VAL-1', standardCost: 10 });
  await commerce.costAccounting.setItemCost({ sku: 'VAL-2', standardCost: 2.5 });
  // A cost record with no inventory item contributes nothing.
  await commerce.costAccounting.setItemCost({ sku: 'GHOST', standardCost: 1000 });

  // 10 x 10.00 + 4 x 2.50 = 110.00
  assert.equal(await commerce.costAccounting.getTotalInventoryValue(), 110);

  // Re-averaging VAL-1 to 11.25 (10 @ 10 + 10 @ 12.5) revalues the 10 on hand.
  await commerce.costAccounting.updateAverageCost('VAL-1', 10, 12.5);
  assert.equal(await commerce.costAccounting.getTotalInventoryValue(), 122.5);

  // Stock adjustments change the valuation through quantity.
  await commerce.inventory.adjust('VAL-2', -4, 'Scrapped');
  assert.equal(await commerce.costAccounting.getTotalInventoryValue(), 112.5);
});

test(
  'setItemCost refuses a negative cost',
  async () => {
    const commerce = new Commerce(':memory:');
    await assert.rejects(
      commerce.costAccounting.setItemCost({ sku: 'NEG-1', standardCost: -1 }),
      (err) => err.code === 'VALIDATION',
    );
    assert.equal(await commerce.costAccounting.getItemCost('NEG-1'), null);
  },
);

test(
  'setItemCost refuses an empty SKU',
  async () => {
    const commerce = new Commerce(':memory:');
    await assert.rejects(
      commerce.costAccounting.setItemCost({ sku: '', standardCost: 1 }),
      (err) => err.code === 'VALIDATION',
    );
    assert.deepEqual(await commerce.costAccounting.listItemCosts(), []);
  },
);

test('non-finite money and quantities are refused with VALIDATION', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.costAccounting.setItemCost({ sku: 'X', standardCost: NaN }),
    (err) => err.code === 'VALIDATION' && /standard cost/i.test(err.message),
  );
  await assert.rejects(
    commerce.costAccounting.setItemCost({ sku: 'X', materialCost: Infinity }),
    (err) => err.code === 'VALIDATION' && /material cost/i.test(err.message),
  );
  await assert.rejects(
    commerce.costAccounting.updateAverageCost('X', NaN, 1),
    (err) => err.code === 'VALIDATION' && /quantity/i.test(err.message),
  );
  await assert.rejects(
    commerce.costAccounting.updateAverageCost('X', 1, NaN),
    (err) => err.code === 'VALIDATION' && /unit cost/i.test(err.message),
  );
  assert.deepEqual(await commerce.costAccounting.listItemCosts(), []);
});
