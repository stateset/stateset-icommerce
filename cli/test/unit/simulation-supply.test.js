import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  inventoryExperimentSchema,
} from '../../src/simulation/schema.js';
import { projectSupply, hashSupplyProjection } from '../../src/simulation/supply.js';
import { projectDemand } from '../../src/simulation/demand.js';
import { getSimulationKernelContract } from '../../src/simulation/contract.js';

const product = {
  sku: 'SUPPLY',
  initialStock: 2,
  unitPrice: '2.50',
  unitCost: '0.100001',
  demand: { min: 0, max: 5 },
  reorderPoint: 1,
  targetStock: 6,
  leadTimeDays: 2,
};
const input = {
  name: 'supply-paths',
  seed: 42,
  days: 30,
  startingCash: '20.00',
  products: [product],
};

test('supply inputs preserve fixed defaults and strictly bound ranges and split receipts', () => {
  const fixed = projectSupply(inventoryPlanSchema.parse(input));
  assert.deepEqual(fixed.products[0].dailyLeadTimeDays, Array(31).fill(2));
  assert.deepEqual(fixed.products[0].dailyFirstReceiptBps, Array(31).fill(10000));
  for (const change of [
    { leadTimeDays: 0 },
    { leadTimeDays: 61 },
    { leadTimeDays: { min: 5, max: 2 } },
    { leadTimeDays: { min: 1, max: 2, code: 'anything' } },
    { delivery: { firstReceiptBps: 10001, remainderDelayDays: 1 } },
    { delivery: { firstReceiptBps: -1, remainderDelayDays: 1 } },
    { delivery: { firstReceiptBps: 0.5, remainderDelayDays: 1 } },
    { delivery: { firstReceiptBps: { min: 9000, max: 8000 }, remainderDelayDays: 1 } },
    { delivery: { firstReceiptBps: 5000, remainderDelayDays: 0 } },
    { delivery: { firstReceiptBps: 5000, remainderDelayDays: 61 } },
    { delivery: { firstReceiptBps: 5000, remainderDelayDays: 1, callback: 'anything' } },
  ])
    assert.equal(
      inventoryPlanSchema.safeParse({ ...input, products: [{ ...product, ...change }] }).success,
      false,
      JSON.stringify(change),
    );
});

test('supply samples are stable by SKU and placement day across policy and product-order changes', () => {
  const variable = {
    ...product,
    leadTimeDays: { min: 1, max: 9 },
    delivery: { firstReceiptBps: { min: 0, max: 10000 }, remainderDelayDays: 3 },
  };
  const plan = inventoryPlanSchema.parse({ ...input, products: [variable] });
  const supply = projectSupply(plan);
  assert.deepEqual(projectSupply(plan), supply);
  assert.equal(supply.generator, 'mulberry32-supply-v1');
  assert.equal(supply.products[0].dailyLeadTimeDays.length, 31);
  assert.ok(new Set(supply.products[0].dailyLeadTimeDays).size > 1);
  assert.ok(supply.products[0].dailyLeadTimeDays.every((days) => days >= 1 && days <= 9));
  assert.ok(supply.products[0].dailyFirstReceiptBps.every((bps) => bps >= 0 && bps <= 10000));
  const extra = { ...variable, sku: 'OTHER' };
  const changed = inventoryPlanSchema.parse({
    ...plan,
    products: [extra, { ...variable, targetStock: 50, demand: { min: 9, max: 9 } }],
  });
  assert.deepEqual(projectSupply(changed).products[1], supply.products[0]);
  assert.deepEqual(
    projectDemand(plan),
    projectDemand(inventoryPlanSchema.parse({ ...plan, products: [product] })),
  );
  const changedLead = projectSupply({
    ...plan,
    products: [{ ...variable, leadTimeDays: { min: 10, max: 20 } }],
  });
  assert.deepEqual(
    changedLead.products[0].dailyFirstReceiptBps,
    supply.products[0].dailyFirstReceiptBps,
  );
  assert.notDeepEqual(projectSupply({ ...plan, seed: 43 }).products, supply.products);
  const longer = projectSupply({ ...plan, days: 40 });
  assert.deepEqual(
    longer.products[0].dailyLeadTimeDays.slice(0, 31),
    supply.products[0].dailyLeadTimeDays,
  );
});

test('supply hashes normalize SKU order and retain differences in receipts and delays', () => {
  const projection = projectSupply(
    inventoryPlanSchema.parse({ ...input, products: [product, { ...product, sku: 'OTHER' }] }),
  );
  assert.equal(
    hashSupplyProjection(projection),
    hashSupplyProjection({ ...projection, products: [...projection.products].reverse() }),
  );
  const changed = structuredClone(projection);
  changed.products[0].dailyFirstReceiptBps[0] = 0;
  assert.notEqual(hashSupplyProjection(projection), hashSupplyProjection(changed));
  changed.products[0].dailyFirstReceiptBps[0] = 10000;
  changed.products[0].remainderDelayDays += 1;
  assert.notEqual(hashSupplyProjection(projection), hashSupplyProjection(changed));
});

test('demand generator v1 retains the previously validated native history path', () => {
  const plan = inventoryPlanSchema.parse({
    ...input,
    days: 14,
    products: [
      {
        ...product,
        sku: 'COFFEE-250G',
        demand: { history: [3, 5, 4, 8, 6, 9, 4] },
        leadTimeDays: { min: 1, max: 4 },
        delivery: { firstReceiptBps: { min: 0, max: 10000 }, remainderDelayDays: 2 },
      },
    ],
  });
  assert.deepEqual(
    projectDemand(plan).products[0].dailyUnits,
    [8, 9, 8, 5, 4, 8, 8, 8, 9, 3, 9, 9, 6, 3],
  );
});

test('explicit supplier schedules require a bounded value for every placement day including day zero in all modes', () => {
  const scheduled = {
    ...input,
    days: 2,
    products: [
      {
        ...product,
        leadTimeDays: { dailyValues: [3, 1, 2] },
        delivery: { firstReceiptBps: { dailyValues: [0, 5000, 10000] }, remainderDelayDays: 1 },
      },
    ],
  };
  assert.equal(inventoryPlanSchema.safeParse(scheduled).success, true);
  assert.equal(
    inventoryPlanComparisonSchema.safeParse([scheduled, { ...scheduled, name: 'other' }]).success,
    true,
  );
  const { seed: _seed, ...policy } = scheduled;
  assert.equal(
    inventoryExperimentSchema.safeParse({ name: 'calendar', seeds: [7, 42], plans: [policy] })
      .success,
    true,
  );
  for (const [field, invalid] of [
    ['leadTimeDays', []],
    ['leadTimeDays', [1, 1]],
    ['leadTimeDays', [1, 1, 1, 1]],
    ['leadTimeDays', [0, 1, 1]],
    ['leadTimeDays', [61, 1, 1]],
    ['leadTimeDays', [1, 1.5, 1]],
    ['leadTimeDays', ['1', 1, 1]],
    ['firstReceiptBps', [10000, 10000]],
    ['leadTimeDays', Array(62).fill(1)],
    ['firstReceiptBps', [0, 5000, 10001]],
    ['firstReceiptBps', [0, -1, 10000]],
    ['firstReceiptBps', [0, 0.5, 10000]],
  ]) {
    const changed = structuredClone(scheduled);
    const condition =
      field === 'leadTimeDays'
        ? changed.products[0].leadTimeDays
        : changed.products[0].delivery.firstReceiptBps;
    condition.dailyValues = invalid;
    assert.equal(
      inventoryPlanSchema.safeParse(changed).success,
      false,
      JSON.stringify({ field, invalid }),
    );
    assert.equal(
      inventoryPlanComparisonSchema.safeParse([scheduled, { ...changed, name: 'bad-calendar' }])
        .success,
      false,
    );
    const { seed: _ignored, ...badPolicy } = changed;
    assert.equal(
      inventoryExperimentSchema.safeParse({
        name: 'bad-calendar',
        seeds: [7, 42],
        plans: [badPolicy],
      }).success,
      false,
    );
  }
  for (const condition of [
    { dailyValues: [1, 1, 1], min: 1, max: 1 },
    { dailyValues: [1, 1, 1], callback: 'code' },
  ])
    assert.equal(
      inventoryPlanSchema.safeParse({
        ...scheduled,
        products: [{ ...scheduled.products[0], leadTimeDays: condition }],
      }).success,
      false,
    );
  const discovered =
    getSimulationKernelContract().schemas.inventoryPlanning.properties.products.items.properties;
  assert.ok(
    discovered.leadTimeDays.anyOf.some((entry) => entry.properties?.dailyValues?.maxItems === 61),
  );
  assert.ok(
    discovered.delivery.properties.firstReceiptBps.anyOf.some(
      (entry) => entry.properties?.dailyValues?.items.maximum === 10000,
    ),
  );
  assert.equal(
    inventoryPlanSchema.safeParse({
      ...scheduled,
      days: 60,
      products: [{ ...product, leadTimeDays: { dailyValues: Array(61).fill(1) } }],
    }).success,
    true,
  );
});

test('supplier calendars keep day order across seeds, isolate arrays and preserve the other sampled stream', () => {
  const plan = inventoryPlanSchema.parse({
    ...input,
    days: 4,
    products: [
      {
        ...product,
        leadTimeDays: { dailyValues: [3, 9, 9, 1, 9] },
        delivery: {
          firstReceiptBps: { dailyValues: [5000, 0, 0, 10000, 0] },
          remainderDelayDays: 1,
        },
      },
    ],
  });
  const projected = projectSupply(plan);
  assert.deepEqual(projected.products[0].dailyLeadTimeDays, [3, 9, 9, 1, 9]);
  assert.deepEqual(projected.products[0].dailyFirstReceiptBps, [5000, 0, 0, 10000, 0]);
  assert.equal(projected.products[0].leadTimeMethod, 'explicit-daily-values');
  assert.equal(projected.products[0].firstReceiptMethod, 'explicit-daily-values');
  assert.deepEqual(projectSupply({ ...plan, seed: 99 }).products, projected.products);
  projected.products[0].dailyLeadTimeDays[0] = 60;
  assert.equal(plan.products[0].leadTimeDays.dailyValues[0], 3);
  const mixed = {
    ...plan,
    products: [
      {
        ...plan.products[0],
        delivery: { firstReceiptBps: { min: 0, max: 10000 }, remainderDelayDays: 1 },
      },
    ],
  };
  const sampled = projectSupply(mixed);
  assert.equal(sampled.products[0].firstReceiptMethod, 'uniform-integer');
  assert.deepEqual(
    sampled.products[0].dailyFirstReceiptBps,
    projectSupply({ ...mixed, products: [{ ...mixed.products[0], leadTimeDays: 1 }] }).products[0]
      .dailyFirstReceiptBps,
  );
  assert.notDeepEqual(
    projectSupply({ ...mixed, seed: 99 }).products[0].dailyFirstReceiptBps,
    sampled.products[0].dailyFirstReceiptBps,
  );
  assert.deepEqual(
    projectDemand(mixed),
    projectDemand({ ...mixed, products: [{ ...mixed.products[0], leadTimeDays: 1 }] }),
  );
});

test('explicit supplier path hashes track placement-day values independently of equivalent fixed inputs', () => {
  const fixed = inventoryPlanSchema.parse({ ...input, days: 2 });
  const explicit = {
    ...fixed,
    products: [
      {
        ...fixed.products[0],
        leadTimeDays: { dailyValues: [2, 2, 2] },
        delivery: {
          firstReceiptBps: { dailyValues: [10000, 10000, 10000] },
          remainderDelayDays: 1,
        },
      },
    ],
  };
  assert.equal(
    hashSupplyProjection(projectSupply(fixed)),
    hashSupplyProjection(projectSupply(explicit)),
  );
  explicit.products[0].leadTimeDays.dailyValues[1] = 3;
  assert.notEqual(
    hashSupplyProjection(projectSupply(fixed)),
    hashSupplyProjection(projectSupply(explicit)),
  );
});

test('sampled supplier generator v1 retains its captured path hash after adding schedules', () => {
  const plan = inventoryPlanSchema.parse({
    ...input,
    products: [
      {
        ...product,
        leadTimeDays: { min: 1, max: 9 },
        delivery: { firstReceiptBps: { min: 0, max: 10000 }, remainderDelayDays: 3 },
      },
    ],
  });
  assert.equal(
    hashSupplyProjection(projectSupply(plan)),
    'b826dd3b6542faeded1fe23394e89b526014323d02f66db706e9d13ed0fb873b',
  );
});
