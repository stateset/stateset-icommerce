import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  commerceScenarioSchema,
  commerceComparisonSchema,
  commerceActionSchema,
  inventoryExperimentSchema,
} from '../../src/simulation/schema.js';
import { getSimulationKernelContract } from '../../src/simulation/contract.js';
import { SIMULATION_LIMITS } from '../../src/simulation/limits.js';
import { projectDemand } from '../../src/simulation/demand.js';
import { reconcilePlan } from '../../src/simulation/reconciliation.js';
import { compareInventoryReports } from '../../src/simulation/comparison.js';
import { projectSupply } from '../../src/simulation/supply.js';

const product = {
  sku: 'COFFEE',
  initialStock: 2,
  unitPrice: '2.50',
  unitCost: '1.25',
  demand: { min: 2, max: 8 },
  reorderPoint: 2,
  targetStock: 6,
  leadTimeDays: 1,
};
const plan = { name: 'coffee', days: 10, startingCash: '20.00', products: [product] };

test('kernel discovery publishes the executable action surface and shared bounds', () => {
  const contract = getSimulationKernelContract();
  assert.deepEqual(contract.limits, SIMULATION_LIMITS);
  assert.deepEqual(
    contract.actions,
    commerceActionSchema.options.map((option) => option.shape.type.value),
  );
  assert.equal(
    contract.schemas.commerce.properties.steps.maxItems,
    contract.limits.maxCommerceActions,
  );
  assert.equal(
    contract.schemas.commerce.properties.inventory.maxItems,
    contract.limits.maxCommerceProducts,
  );
  assert.equal(
    contract.schemas.inventoryPlanning.properties.days.maximum,
    contract.limits.maxPlanningDays,
  );
  assert.equal(
    contract.schemas.inventoryPlanning.properties.products.maxItems,
    contract.limits.maxPlanningProducts,
  );
  assert.equal(contract.schemas.commerce.additionalProperties, false);
  assert.equal(contract.limits.maxInputBytes, 131072);
  assert.equal(contract.limits.maxInputNodes, 16384);
  assert.equal(contract.limits.maxInputDepth, 32);
  const dailyUnits =
    contract.schemas.inventoryPlanning.properties.products.items.properties.demand.anyOf.find(
      (variant) => variant.properties.dailyUnits,
    )?.properties.dailyUnits;
  assert.equal(dailyUnits.minItems, 1);
  assert.equal(dailyUnits.maxItems, SIMULATION_LIMITS.maxPlanningDays);
  assert.equal(dailyUnits.items.maximum, 10000);
  assert.equal(
    contract.schemas.inventoryExperiment.properties.seeds.maxItems,
    contract.limits.maxExperimentSeeds,
  );
  assert.equal(
    Object.hasOwn(contract.schemas.inventoryExperiment.properties.plans.items.properties, 'seed'),
    false,
  );
  assert.equal(contract.actions.length, 11);
  const goals = contract.schemas.inventoryExperiment.properties.goals;
  assert.equal(goals.additionalProperties, false);
  assert.equal(goals.properties.minFillRateBps.maximum, 10000);
  assert.equal(goals.properties.minEndingCash.type, 'string');
  assert.equal(goals.properties.minCashBalance.type, 'string');
  contract.limits.maxActiveWorkers = 999;
  assert.equal(getSimulationKernelContract().limits.maxActiveWorkers, 2);
});

test('procurement actions enforce exact money, known SKUs and unique aliases', () => {
  const input = {
    name: 'purchase',
    inventory: [{ sku: 'COFFEE', quantity: 0, unitPrice: '2.50' }],
    steps: [
      {
        type: 'purchase_inventory',
        id: 'supply',
        items: [{ sku: 'COFFEE', quantity: 3, unitCost: '0.100001' }],
      },
    ],
  };
  assert.equal(commerceScenarioSchema.safeParse(input).success, true);
  for (const change of [
    { unitCost: 0.1 },
    { unitCost: '-1' },
    { quantity: 0 },
    { sku: 'UNKNOWN' },
    { unitCost: '0.0000001' },
  ]) {
    const invalid = {
      ...input,
      steps: [{ ...input.steps[0], items: [{ ...input.steps[0].items[0], ...change }] }],
    };
    assert.equal(commerceScenarioSchema.safeParse(invalid).success, false);
  }
  assert.equal(
    commerceScenarioSchema.safeParse({ ...input, steps: [...input.steps, input.steps[0]] }).success,
    false,
  );
});

test('planning inputs apply defaults, preserve exact costs and refuse unsafe or impossible values', () => {
  const parsed = inventoryPlanSchema.parse(plan);
  assert.equal(parsed.seed, 1);
  assert.equal(parsed.currency, 'USD');
  assert.equal(parsed.products[0].unitCost, '1.25');
  for (const input of [
    { ...plan, database: '/operator/store.db' },
    { ...plan, seed: -1 },
    { ...plan, seed: 4294967296 },
    { ...plan, days: 0 },
    { ...plan, days: 61 },
    { ...plan, startingCash: 20 },
    { ...plan, products: [product, product] },
    { ...plan, products: [{ ...product, unitPrice: '1.001' }] },
    { ...plan, products: [{ ...product, unitPrice: '0.00' }] },
    { ...plan, products: [{ ...product, unitCost: 1.25 }] },
    { ...plan, products: [{ ...product, targetStock: 2 }] },
    { ...plan, products: [{ ...product, demand: { min: 9, max: 2 } }] },
    { ...plan, products: [{ ...product, demand: { history: [1] } }] },
    { ...plan, products: [{ ...product, demand: { history: [1, -1] } }] },
    { ...plan, products: [{ ...product, command: 'shell command' }] },
  ])
    assert.equal(inventoryPlanSchema.safeParse(input).success, false, JSON.stringify(input));
  assert.equal(
    inventoryPlanSchema.safeParse({ ...plan, products: [{ ...product, unitCost: '0.100001' }] })
      .success,
    true,
  );
});

test('planning work limits apply to the whole comparison', () => {
  const products = Array.from({ length: 3 }, (_, index) => ({ ...product, sku: `SKU-${index}` }));
  assert.equal(inventoryPlanSchema.safeParse({ ...plan, days: 40, products }).success, true);
  assert.equal(inventoryPlanSchema.safeParse({ ...plan, days: 41, products }).success, false);
  const first = { ...plan, days: 30, products };
  assert.equal(
    inventoryPlanComparisonSchema.safeParse([first, { ...first, name: 'second' }]).success,
    true,
  );
  assert.equal(
    inventoryPlanComparisonSchema.safeParse([first, { ...first, name: 'second', days: 31 }])
      .success,
    false,
  );
  assert.equal(inventoryPlanComparisonSchema.safeParse([first, first]).success, false);
  assert.equal(
    inventoryPlanComparisonSchema.safeParse([first, { ...first, name: 'euros', currency: 'EUR' }])
      .success,
    false,
  );
});

test('commerce input bounds reject ambiguous identities and executable payloads', () => {
  const scenario = {
    name: 'checkout',
    inventory: [{ sku: 'COFFEE', unitPrice: '2.50', quantity: 1 }],
    steps: [{ type: 'place_order', id: 'sale', items: [{ sku: 'COFFEE', quantity: 1 }] }],
  };
  assert.equal(commerceScenarioSchema.parse(scenario).steps[0].expect, 'success');
  for (const input of [
    { ...scenario, code: 'shell command' },
    { ...scenario, inventory: [scenario.inventory[0], scenario.inventory[0]] },
    { ...scenario, inventory: [{ ...scenario.inventory[0], unitPrice: '2.501' }] },
    { ...scenario, steps: [...scenario.steps, scenario.steps[0]] },
    { ...scenario, steps: [{ ...scenario.steps[0], items: [{ sku: 'UNKNOWN', quantity: 1 }] }] },
    { ...scenario, steps: [{ type: 'eval', code: 'shell command' }] },
  ])
    assert.equal(commerceScenarioSchema.safeParse(input).success, false);
  const steps = Array.from({ length: 50 }, () => ({ type: 'restock', sku: 'COFFEE', quantity: 1 }));
  const first = { ...scenario, steps };
  assert.equal(
    commerceComparisonSchema.safeParse([first, { ...first, name: 'second' }]).success,
    true,
  );
  assert.equal(
    commerceComparisonSchema.safeParse([
      first,
      { ...first, name: 'second', steps: [...steps, steps[0]] },
    ]).success,
    false,
  );
});

test('seeded demand is stable by SKU across policy, order and product-set changes', () => {
  const base = inventoryPlanSchema.parse({ ...plan, days: 60, seed: 42 });
  const first = projectDemand(base);
  assert.deepEqual(projectDemand(base), first);
  assert.equal(first.generator, 'mulberry32-sku-v1');
  const extra = { ...product, sku: 'OTHER', demand: { history: [0, 2, 5, 9] } };
  for (const products of [
    [extra, { ...product, targetStock: 50 }],
    [{ ...product, leadTimeDays: 30 }, extra],
  ]) {
    const modified = projectDemand(inventoryPlanSchema.parse({ ...base, products }));
    assert.deepEqual(
      modified.products.find((item) => item.sku === product.sku),
      first.products[0],
    );
  }
  assert.notDeepEqual(
    projectDemand({ ...base, seed: 43 }).products[0].dailyUnits,
    first.products[0].dailyUnits,
  );
  assert.ok(first.products[0].dailyUnits.every((units) => units >= 2 && units <= 8));
  assert.equal(first.products[0].expectedDailyUnits, 5);
  assert.equal(first.products[0].expectedHorizonUnits, 300);
  assert.equal(
    first.products[0].projectedUnits,
    first.products[0].dailyUnits.reduce((sum, units) => sum + units, 0),
  );
});

test('historical demand resamples only supplied observations and exposes the conditional mean', () => {
  const history = [0, 2, 4];
  const projection = projectDemand(
    inventoryPlanSchema.parse({ ...plan, products: [{ ...product, demand: { history } }] }),
  );
  const [result] = projection.products;
  assert.equal(result.method, 'empirical-history-resampling');
  assert.equal(result.expectedDailyUnits, 2);
  assert.equal(result.expectedHorizonUnits, 20);
  assert.ok(result.dailyUnits.every((units) => history.includes(units)));
});

test('explicit demand schedules enforce one bounded integer entry per virtual day in every input mode', () => {
  const scheduled = {
    ...plan,
    days: 4,
    products: [{ ...product, demand: { dailyUnits: [0, 0, 8, 2] } }],
  };
  assert.equal(inventoryPlanSchema.safeParse(scheduled).success, true);
  const experiment = { name: 'forecast-input', seeds: [7, 42], plans: [scheduled] };
  assert.equal(inventoryExperimentSchema.safeParse(experiment).success, true);
  for (const dailyUnits of [
    [],
    [1],
    [0, 0, 8],
    [0, 0, 8, 2, 1],
    [0, 0, -1, 2],
    [0, 0, 1.5, 2],
    [0, 0, 10001, 2],
    ['0', 0, 8, 2],
  ]) {
    const invalid = { ...scheduled, products: [{ ...product, demand: { dailyUnits } }] };
    const validation = inventoryPlanSchema.safeParse(invalid);
    assert.equal(validation.success, false, JSON.stringify(dailyUnits));
    assert.equal(
      inventoryPlanComparisonSchema.safeParse([scheduled, { ...invalid, name: 'other' }]).success,
      false,
    );
    assert.equal(
      inventoryExperimentSchema.safeParse({ ...experiment, plans: [invalid] }).success,
      false,
    );
  }
  const invalidLength = inventoryPlanSchema.safeParse({ ...scheduled, days: 5 });
  assert.ok(
    invalidLength.error.issues.some(
      (issue) => issue.path.join('.') === 'products.0.demand.dailyUnits',
    ),
  );
  for (const demand of [
    { dailyUnits: [0, 0, 8, 2], min: 0, max: 8 },
    { dailyUnits: [0, 0, 8, 2], history: [0, 8] },
  ])
    assert.equal(
      inventoryPlanSchema.safeParse({ ...scheduled, products: [{ ...product, demand }] }).success,
      false,
    );
  assert.equal(
    inventoryPlanSchema.safeParse({
      ...scheduled,
      days: 60,
      products: [{ ...product, demand: { dailyUnits: Array(60).fill(10000) } }],
    }).success,
    true,
  );
  assert.equal(
    inventoryPlanSchema.safeParse({
      ...scheduled,
      days: 60,
      products: [{ ...product, demand: { dailyUnits: Array(61).fill(1) } }],
    }).success,
    false,
  );
});

test('explicit forecast replay preserves day order, ignores demand seeds and isolates input arrays', () => {
  const dailyUnits = [0, 1, 0, 8, 0, 2];
  const input = inventoryPlanSchema.parse({
    ...plan,
    days: dailyUnits.length,
    seed: 7,
    products: [{ ...product, demand: { dailyUnits } }],
  });
  const first = projectDemand(input).products[0];
  assert.equal(first.method, 'explicit-daily-units');
  assert.deepEqual(first.dailyUnits, dailyUnits);
  assert.equal(first.projectedUnits, 11);
  assert.equal(first.expectedHorizonUnits, 11);
  assert.deepEqual(projectDemand({ ...input, seed: 99 }).products[0], first);
  const more = { ...input, products: [{ ...product, sku: 'SAMPLED' }, ...input.products] };
  assert.deepEqual(projectDemand(more).products[1], first);
  first.dailyUnits[0] = 100;
  assert.deepEqual(input.products[0].demand.dailyUnits, dailyUnits);
  assert.deepEqual(projectDemand(input).products[0].dailyUnits, dailyUnits);
});

test('scheduled horizon totals stay exact even when the daily mean is a repeating binary fraction', () => {
  const dailyUnits = [7, ...Array(24).fill(0)];
  const input = inventoryPlanSchema.parse({
    ...plan,
    days: 25,
    products: [{ ...product, demand: { dailyUnits } }],
  });
  const [projection] = projectDemand(input).products;
  assert.equal(projection.expectedHorizonUnits, 7);
  assert.equal(projection.projectedUnits, 7);
  assert.equal(projection.expectedDailyUnits, 7 / 25);
});

function balances() {
  return {
    startingCash: '20.00',
    openingInventoryValue: '2.50',
    products: [product],
    inventory: [
      {
        sku: product.sku,
        onHand: 4,
        available: 4,
        allocated: 0,
        onOrder: 0,
        purchasedUnits: 8,
        receivedUnits: 8,
        sold: 6,
        demanded: 6,
        lostSales: 0,
      },
    ],
    summary: {
      revenue: '15.00',
      purchasingSpend: '10.00',
      endingCash: '25.00',
      endingInventoryValue: '5.00',
      inTransitInventoryValue: '0.00',
      costOfGoodsSold: '7.50',
      grossMargin: '7.50',
    },
  };
}

test('reconciliation binds stock, cash and inventory value to one financial result', () => {
  assert.deepEqual(reconcilePlan(balances()), {
    stockBalanced: true,
    cashBalanced: true,
    inventoryValueBalanced: true,
    openingAssets: '22.50',
    endingAssets: '30.00',
  });
  for (const field of [
    'onHand',
    'available',
    'allocated',
    'onOrder',
    'purchasedUnits',
    'receivedUnits',
    'sold',
    'lostSales',
  ]) {
    const broken = balances();
    broken.inventory[0][field] += 1;
    assert.throws(() => reconcilePlan(broken), /inventory reconciliation/);
  }
  for (const field of [
    'endingCash',
    'revenue',
    'purchasingSpend',
    'endingInventoryValue',
    'inTransitInventoryValue',
    'grossMargin',
    'costOfGoodsSold',
  ]) {
    const broken = balances();
    broken.summary[field] = '123.01';
    assert.throws(() => reconcilePlan(broken), /reconciliation/);
  }
});

test('financial reconciliation preserves cents above JavaScript safe integer range', () => {
  const input = {
    startingCash: '9007199254740993.01',
    openingInventoryValue: '0.00',
    products: [{ ...product, initialStock: 0, unitCost: '0.10', unitPrice: '0.29' }],
    inventory: [
      {
        sku: product.sku,
        onHand: 1,
        available: 1,
        allocated: 0,
        onOrder: 0,
        purchasedUnits: 2,
        receivedUnits: 2,
        sold: 1,
        demanded: 1,
        lostSales: 0,
      },
    ],
    summary: {
      revenue: '0.29',
      purchasingSpend: '0.20',
      endingCash: '9007199254740993.10',
      endingInventoryValue: '0.10',
      inTransitInventoryValue: '0.00',
      costOfGoodsSold: '0.10',
      grossMargin: '0.19',
    },
  };
  assert.equal(reconcilePlan(input).endingAssets, '9007199254740993.20');
  input.summary.endingCash = '9007199254740993.11';
  assert.throws(() => reconcilePlan(input), /cash reconciliation/);
});

function comparisonReports() {
  const summary = {
    demanded: 3,
    sold: 2,
    lostSales: 1,
    orders: 2,
    purchaseOrders: 1,
    partialReceipts: 0,
    missedDeliveries: 0,
    constraintBlockedPurchases: 0,
    reserveBlockedPurchases: 0,
    deferredPurchaseReviews: 0,
    roundedPurchaseOrders: 0,
    extraPurchasedUnits: 0,
    revenue: '5.00',
    lostRevenue: '2.50',
    purchasingSpend: '1.25',
    startingCash: '20.00',
    endingCash: '9007199254740993.01',
    minimumCash: '9007199254740992.99',
    grossMargin: '2.50',
    endingInventoryValue: '1.25',
    inTransitInventoryValue: '0.00',
  };
  const baseline = {
    name: 'baseline',
    currency: 'USD',
    days: 2,
    summary,
    assumptions: { products: [product, { ...product, sku: 'TEA' }] },
    supplyProjection: projectSupply({
      ...plan,
      seed: 42,
      days: 2,
      products: [product, { ...product, sku: 'TEA' }],
    }),
    demandForecast: {
      products: [
        { sku: 'COFFEE', dailyUnits: [1, 2] },
        { sku: 'TEA', dailyUnits: [0, 0] },
      ],
    },
  };
  const alternative = {
    ...baseline,
    name: 'alternative',
    summary: { ...summary, sold: 3, lostSales: 0, endingCash: '9007199254740993.02' },
    demandForecast: { products: [...baseline.demandForecast.products].reverse() },
  };
  return [baseline, alternative];
}

test('policy comparisons retain exact money deltas and identify mismatched demand or horizons', () => {
  const [baseline, alternative] = comparisonReports();
  const comparison = compareInventoryReports([baseline, alternative]);
  assert.equal(comparison.sameHorizon, true);
  assert.equal(comparison.sameDemandPath, true);
  assert.equal(comparison.sameCashReserve, true);
  assert.equal(
    compareInventoryReports([
      baseline,
      { ...alternative, assumptions: { ...alternative.assumptions, cashReserve: '1.00' } },
    ]).sameCashReserve,
    false,
  );
  assert.equal(comparison.rows[1].deltaFromBaseline.endingCash, '0.01');
  assert.equal(comparison.rows[1].deltaFromBaseline.sold, 1);
  assert.equal(comparison.rows[1].deltaFromBaseline.lostSales, -1);
  const differentDemand = {
    ...alternative,
    demandForecast: { products: [{ sku: 'COFFEE', dailyUnits: [2, 2] }] },
  };
  assert.equal(compareInventoryReports([baseline, differentDemand]).sameDemandPath, false);
  assert.equal(compareInventoryReports([baseline, { ...alternative, days: 3 }]).sameHorizon, false);
});

test('single-run comparisons disclose each alternative conditions without hiding a matched policy behind a third mismatch', () => {
  const [baseline, matched] = comparisonReports();
  const changed = {
    ...matched,
    name: 'different-capital',
    summary: { ...matched.summary, startingCash: '20.000001' },
    assumptions: {
      ...matched.assumptions,
      cashReserve: '1.00',
      products: matched.assumptions.products.map((item) => ({
        ...item,
        initialStock: item.initialStock + 1,
        unitCost: '1.250001',
      })),
    },
  };
  const before = structuredClone([baseline, matched, changed]);
  const result = compareInventoryReports([baseline, matched, changed]);
  assert.equal(result.sameOpeningCash, false);
  assert.equal(result.sameOpeningInventory, false);
  assert.equal(result.sameUnitEconomics, false);
  assert.equal(result.sameCashReserve, false);
  assert.equal(result.sameProductPriority, true);
  assert.deepEqual(result.rows[0].comparisonToBaseline.changedConditions, []);
  assert.deepEqual(result.rows[1].comparisonToBaseline.changedConditions, []);
  assert.deepEqual(result.rows[2].comparisonToBaseline.changedConditions, [
    'openingCash',
    'cashReserve',
    'openingInventory',
    'unitEconomics',
  ]);
  assert.equal(result.rows[2].comparisonToBaseline.sameDemandPaths, true);
  assert.equal(result.rows[2].comparisonToBaseline.sameSupplyPaths, true);
  assert.deepEqual([baseline, matched, changed], before);
});

test('comparison conditions normalize exact money and SKU facts while disclosing product priority separately', () => {
  const [baseline, alternative] = comparisonReports();
  const normalized = {
    ...alternative,
    summary: { ...alternative.summary, startingCash: '20.000000' },
    assumptions: {
      ...alternative.assumptions,
      cashReserve: '0.000000',
      products: alternative.assumptions.products.map((item) => ({
        ...item,
        unitCost: '1.25000',
        unitPrice: '2.500000',
      })),
    },
  };
  assert.deepEqual(
    compareInventoryReports([baseline, normalized]).rows[1].comparisonToBaseline.changedConditions,
    [],
  );
  const reordered = {
    ...normalized,
    assumptions: {
      ...normalized.assumptions,
      products: [...normalized.assumptions.products].reverse(),
    },
  };
  const result = compareInventoryReports([baseline, reordered]);
  assert.equal(result.sameOpeningInventory, true);
  assert.equal(result.sameUnitEconomics, true);
  assert.equal(result.sameProductPriority, false);
  assert.deepEqual(result.rows[1].comparisonToBaseline.changedConditions, ['productPriority']);
  const large = {
    ...baseline,
    summary: { ...baseline.summary, startingCash: '9007199254740993.000001' },
  };
  const different = {
    ...alternative,
    summary: { ...alternative.summary, startingCash: '9007199254740993.000002' },
  };
  assert.equal(compareInventoryReports([large, different]).sameOpeningCash, false);
});

test('reconciliation rejects offsetting asset errors and missing native inventory rows', () => {
  const offset = balances();
  offset.summary.endingInventoryValue = '4.00';
  offset.summary.inTransitInventoryValue = '1.00';
  assert.throws(() => reconcilePlan(offset), /inventory-value reconciliation/);
  const missing = balances();
  missing.inventory = [];
  assert.throws(() => reconcilePlan(missing), /exactly one row/);
  const duplicate = balances();
  duplicate.products.push({ ...product, sku: 'OTHER' });
  duplicate.inventory.push({ ...duplicate.inventory[0] });
  assert.throws(() => reconcilePlan(duplicate), /exactly one row/);
});
