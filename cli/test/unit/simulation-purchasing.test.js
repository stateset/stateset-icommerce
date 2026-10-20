import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  inventoryExperimentSchema,
} from '../../src/simulation/schema.js';
import { decideReplenishment } from '../../src/simulation/purchasing.js';
import { projectDemand } from '../../src/simulation/demand.js';
import { projectSupply } from '../../src/simulation/supply.js';
import { getSimulationKernelContract } from '../../src/simulation/contract.js';

const product = {
  sku: 'PACK',
  initialStock: 2,
  unitPrice: '2.50',
  unitCost: '0.100001',
  demand: { min: 1, max: 3 },
  reorderPoint: 2,
  targetStock: 3,
  leadTimeDays: 1,
};
const plan = { name: 'case-packs', seed: 42, days: 4, startingCash: '20.00', products: [product] };
const packed = { ...product, purchasing: { minimumOrderQuantity: 5, orderMultiple: 4 } };

test('purchasing defaults retain exact target sizing and refuse invalid or oversized constraints', () => {
  const defaults = inventoryPlanSchema.parse({
    ...plan,
    products: [{ ...product, purchasing: {} }],
  });
  assert.deepEqual(defaults.products[0].purchasing, { minimumOrderQuantity: 1, orderMultiple: 1 });
  const decision = decideReplenishment(product, 2, '20.00');
  assert.equal(decision.quantity, 1);
  assert.equal(decision.extraUnits, 0);
  assert.equal(decision.spend, '0.100001');
  assert.deepEqual(decideReplenishment(product, 3, '0'), { kind: 'none' });
  for (const purchasing of [
    { minimumOrderQuantity: 0 },
    { orderMultiple: 0 },
    { orderMultiple: 1.5 },
    { maximumInventoryPosition: 0 },
    { maximumInventoryPosition: 1000001 },
    { minimumOrderQuantity: 1000001 },
    { minimumOrderQuantity: 1000000, orderMultiple: 999999 },
    { callback: 'anything' },
  ])
    assert.equal(
      inventoryPlanSchema.safeParse({ ...plan, products: [{ ...product, purchasing }] }).success,
      false,
      JSON.stringify(purchasing),
    );
  assert.equal(
    inventoryPlanSchema.safeParse({
      ...plan,
      products: [
        { ...product, purchasing: { minimumOrderQuantity: 1000000, orderMultiple: 1000000 } },
      ],
    }).success,
    true,
  );
  const schema =
    getSimulationKernelContract().schemas.inventoryPlanning.properties.products.items.properties
      .purchasing;
  assert.equal(schema.properties.minimumOrderQuantity.minimum, 1);
  assert.equal(schema.properties.orderMultiple.default, 1);
  assert.equal(schema.additionalProperties, false);
});

test('MOQ and pack rounding apply before exact affordability and inbound-aware capacity checks', () => {
  const decision = decideReplenishment(packed, 2, '0.800008');
  assert.equal(decision.kind, 'purchase');
  assert.equal(decision.requestedQuantity, 1);
  assert.equal(decision.quantity, 8);
  assert.equal(decision.extraUnits, 7);
  assert.equal(decision.spend, '0.800008');
  assert.equal(decision.requiredPosition, 10);
  assert.equal(decideReplenishment(packed, 2, '0.80').kind, 'budget-blocked');
  const constrained = {
    ...packed,
    purchasing: { ...packed.purchasing, maximumInventoryPosition: 9 },
  };
  const refusal = decideReplenishment(constrained, 2, '0');
  assert.equal(refusal.kind, 'constraint-blocked');
  assert.equal(refusal.reason, 'maximum_inventory_position');
  assert.equal(refusal.maximumInventoryPosition, 9);
  assert.equal(refusal.requiredPosition, 10);
  assert.equal(
    decideReplenishment(
      { ...constrained, purchasing: { ...constrained.purchasing, maximumInventoryPosition: 10 } },
      2,
      '0.800008',
    ).kind,
    'purchase',
  );
  assert.throws(() => decideReplenishment(packed, -1, '20'), /Native inventory position/);
});

test('sizing constraints preserve shared demand and supply paths for policy comparisons', () => {
  const baseline = inventoryPlanSchema.parse(plan);
  const changed = inventoryPlanSchema.parse({ ...plan, cashReserve: '18.00', products: [packed] });
  assert.deepEqual(projectDemand(baseline), projectDemand(changed));
  assert.deepEqual(projectSupply(baseline), projectSupply(changed));
});

test('cash reserves use optional exact amounts in every planning schema without changing omitted defaults', () => {
  assert.equal(Object.hasOwn(inventoryPlanSchema.parse(plan), 'cashReserve'), false);
  const reserved = { ...plan, cashReserve: '20.000001' };
  assert.equal(inventoryPlanSchema.parse(reserved).cashReserve, '20.000001');
  assert.equal(
    inventoryPlanComparisonSchema.parse([plan, { ...reserved, name: 'reserved' }])[1].cashReserve,
    '20.000001',
  );
  const { seed: _seed, ...policy } = reserved;
  assert.equal(
    inventoryExperimentSchema.parse({ name: 'reserves', seeds: [7, 42], plans: [policy] }).plans[0]
      .cashReserve,
    '20.000001',
  );
  for (const cashReserve of [-1, 1, '-0.01', '0.0000001', '1e3', '1000000000000000000', null])
    assert.equal(
      inventoryPlanSchema.safeParse({ ...plan, cashReserve }).success,
      false,
      String(cashReserve),
    );
  const discovered = getSimulationKernelContract().schemas.inventoryPlanning.properties.cashReserve;
  assert.equal(discovered.type, 'string');
  assert.match(discovered.description, /shared cash/);
});

test('whole rounded purchases preserve an exact shared cash reserve after capacity and actual-cash checks', () => {
  const allowed = decideReplenishment(packed, 2, '0.900009', '0.100001');
  assert.equal(allowed.kind, 'purchase');
  assert.equal(allowed.quantity, 8);
  const reserved = decideReplenishment(packed, 2, '0.900008', '0.100001');
  assert.equal(reserved.kind, 'reserve-blocked');
  assert.equal(reserved.quantity, 8);
  assert.equal(reserved.spend, '0.800008');
  assert.equal(reserved.cashReserve, '0.100001');
  assert.equal(reserved.spendableCash, '0.800007');
  assert.equal(decideReplenishment(packed, 2, '0.80', '0.10').kind, 'budget-blocked');
  assert.equal(
    decideReplenishment(
      { ...packed, purchasing: { ...packed.purchasing, maximumInventoryPosition: 9 } },
      2,
      '0',
      '1',
    ).kind,
    'constraint-blocked',
  );
  assert.equal(decideReplenishment(packed, 2, '0.900009', '1.00').spendableCash, '0.00');
  assert.equal(decideReplenishment({ ...packed, unitCost: '0' }, 2, '0', '1').kind, 'purchase');
  const large = { ...product, unitCost: '9007199254740993.000001' };
  assert.equal(decideReplenishment(large, 2, '9007199254740994.000001', '1.00').kind, 'purchase');
  assert.equal(
    decideReplenishment(large, 2, '9007199254740994.000001', '1.000001').kind,
    'reserve-blocked',
  );
  assert.deepEqual(
    decideReplenishment(product, 2, '20.00', '0'),
    decideReplenishment(product, 2, '20.00'),
  );
});

test('purchase review intervals are optional bounded integers in plans, comparisons and experiments', () => {
  assert.equal(Object.hasOwn(inventoryPlanSchema.parse(plan).products[0], 'purchasing'), false);
  const scheduled = { ...plan, products: [{ ...product, purchasing: { reviewIntervalDays: 7 } }] };
  assert.equal(inventoryPlanSchema.parse(scheduled).products[0].purchasing.reviewIntervalDays, 7);
  assert.equal(
    inventoryPlanComparisonSchema.parse([plan, { ...scheduled, name: 'weekly' }])[1].products[0]
      .purchasing.reviewIntervalDays,
    7,
  );
  const { seed: _seed, ...policy } = scheduled;
  assert.equal(
    inventoryExperimentSchema.parse({ name: 'weekly', seeds: [7, 42], plans: [policy] }).plans[0]
      .products[0].purchasing.reviewIntervalDays,
    7,
  );
  for (const reviewIntervalDays of [0, -1, 61, 1.5, '7', null]) {
    const invalid = { ...plan, products: [{ ...product, purchasing: { reviewIntervalDays } }] };
    assert.equal(inventoryPlanSchema.safeParse(invalid).success, false);
    assert.equal(
      inventoryPlanComparisonSchema.safeParse([plan, { ...invalid, name: 'other' }]).success,
      false,
    );
    const { seed: _ignored, ...badPolicy } = invalid;
    assert.equal(
      inventoryExperimentSchema.safeParse({ name: 'invalid', seeds: [7, 42], plans: [badPolicy] })
        .success,
      false,
    );
  }
  assert.equal(
    inventoryPlanSchema.parse({
      ...scheduled,
      products: [{ ...product, purchasing: { reviewIntervalDays: 60 } }],
    }).products[0].purchasing.reviewIntervalDays,
    60,
  );
  const discovered =
    getSimulationKernelContract().schemas.inventoryPlanning.properties.products.items.properties
      .purchasing.properties.reviewIntervalDays;
  assert.equal(discovered.minimum, 1);
  assert.equal(discovered.maximum, 60);
  assert.equal(Object.hasOwn(discovered, 'default'), false);
  assert.deepEqual(
    projectDemand(inventoryPlanSchema.parse(scheduled)),
    projectDemand(inventoryPlanSchema.parse(plan)),
  );
  assert.deepEqual(
    projectSupply(inventoryPlanSchema.parse(scheduled)),
    projectSupply(inventoryPlanSchema.parse(plan)),
  );
});

test('scheduled reviews defer only low-stock attempts before sizing, capacity and cash checks', () => {
  const scheduled = {
    ...packed,
    purchasing: { ...packed.purchasing, reviewIntervalDays: 3, maximumInventoryPosition: 9 },
  };
  assert.deepEqual(decideReplenishment(scheduled, 2, '0', '1', 1), {
    kind: 'review-deferred',
    inventoryPosition: 2,
    reviewIntervalDays: 3,
    nextReviewDay: 3,
  });
  assert.equal(decideReplenishment(scheduled, 2, '0', '1', 2).nextReviewDay, 3);
  assert.equal(decideReplenishment(scheduled, 2, '0', '1', 4).nextReviewDay, 6);
  assert.equal(decideReplenishment(scheduled, 2, '0', '1', 0).kind, 'constraint-blocked');
  assert.equal(decideReplenishment(scheduled, 2, '0', '1', 3).kind, 'constraint-blocked');
  assert.deepEqual(decideReplenishment(scheduled, 3, '0', '1', 1), { kind: 'none' });
  const affordable = { ...packed, purchasing: { ...packed.purchasing, reviewIntervalDays: 3 } };
  assert.equal(decideReplenishment(affordable, 2, '0.900009', '0.100001', 3).kind, 'purchase');
  assert.equal(
    decideReplenishment(affordable, 2, '0.900008', '0.100001', 3).kind,
    'reserve-blocked',
  );
  assert.equal(decideReplenishment(affordable, 2, '0', '0', 3).kind, 'budget-blocked');
  const long = { ...product, purchasing: { reviewIntervalDays: 59 } };
  assert.equal(decideReplenishment(long, 2, '20', '0', 60).nextReviewDay, 118);
  for (let day = 0; day <= 60; day += 1) {
    assert.deepEqual(
      decideReplenishment(product, 2, '20', '0', day),
      decideReplenishment(product, 2, '20'),
    );
    assert.deepEqual(
      decideReplenishment({ ...product, purchasing: { reviewIntervalDays: 1 } }, 2, '20', '0', day),
      decideReplenishment(product, 2, '20'),
    );
  }
  for (const day of [-1, 61, 0.5, NaN])
    assert.throws(() => decideReplenishment(product, 2, '20', '0', day), /review day/);
});
