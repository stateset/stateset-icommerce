import { test } from 'node:test';
import assert from 'node:assert/strict';
import { inventoryExperimentSchema } from '../../src/simulation/schema.js';
import { summarizeInventoryExperiment } from '../../src/simulation/experiment.js';
import { addDecimals } from '../../src/utils/exact-decimal.js';
import { projectSupply } from '../../src/simulation/supply.js';
import { serviceLevel, evaluateInventoryGoals } from '../../src/simulation/evaluation.js';
import { getSimulationKernelContract } from '../../src/simulation/contract.js';

const product = {
  sku: 'SIM',
  initialStock: 2,
  unitPrice: '2.50',
  unitCost: '1.25',
  demand: { min: 0, max: 3 },
  reorderPoint: 1,
  targetStock: 6,
  leadTimeDays: 1,
};
const policy = { name: 'baseline', days: 2, startingCash: '20.00', products: [product] };
const experiment = {
  name: 'policy-variation',
  plans: [policy, { ...policy, name: 'alternative', products: [{ ...product, leadTimeDays: 2 }] }],
  seeds: [7, 42, 99],
};

test('experiments reject duplicate seeds, per-plan seeds, ambiguous identities and aggregate overwork', () => {
  const parsed = inventoryExperimentSchema.parse(experiment);
  assert.equal(parsed.plans[0].currency, 'USD');
  assert.equal(Object.hasOwn(parsed.plans[0], 'seed'), false);
  for (const input of [
    { ...experiment, database: '/operator/store.db' },
    { ...experiment, seeds: [1] },
    { ...experiment, seeds: [1, 1] },
    { ...experiment, seeds: [-1, 1] },
    { ...experiment, seeds: [0, 4294967296] },
    { ...experiment, seeds: Array.from({ length: 9 }, (_, index) => index) },
    { ...experiment, plans: [{ ...policy, seed: 1 }] },
    { ...experiment, plans: [policy, policy] },
    { ...experiment, plans: [policy, { ...policy, name: 'eur', currency: 'EUR' }] },
    { ...experiment, plans: [{ ...policy, products: [product, product] }] },
    { ...experiment, plans: [{ ...policy, days: 61 }] },
  ])
    assert.equal(inventoryExperimentSchema.safeParse(input).success, false, JSON.stringify(input));
  const three = Array.from({ length: 3 }, (_, index) => ({
    ...policy,
    name: `policy-${index}`,
    days: 1,
  }));
  assert.equal(
    inventoryExperimentSchema.safeParse({ ...experiment, plans: three, seeds: [0, 1, 2, 3] })
      .success,
    true,
  );
  assert.equal(
    inventoryExperimentSchema.safeParse({ ...experiment, plans: three, seeds: [0, 1, 2, 3, 4] })
      .success,
    false,
  );
  const products = Array.from({ length: 3 }, (_, index) => ({ ...product, sku: `SIM-${index}` }));
  assert.equal(
    inventoryExperimentSchema.safeParse({
      ...experiment,
      seeds: [0, 1],
      plans: [{ ...policy, days: 30, products }],
    }).success,
    true,
  );
  assert.equal(
    inventoryExperimentSchema.safeParse({
      ...experiment,
      seeds: [0, 1],
      plans: [{ ...policy, days: 31, products }],
    }).success,
    false,
  );
});

function reports() {
  const cash = ['9007199254740993.01', '9007199254740993.02', '9007199254740993.05'];
  const deltas = ['0.01', '-0.02', '0.03'];
  return inventoryExperimentSchema.parse(experiment).plans.flatMap((plan, policyIndex) =>
    experiment.seeds.map((seed, index) => ({
      name: plan.name,
      seed,
      days: plan.days,
      currency: plan.currency,
      demandForecast: { products: [{ sku: 'SIM', dailyUnits: [index, index + 1] }] },
      supplyProjection: projectSupply({ ...plan, seed }),
      summary: {
        demanded: 5,
        sold: policyIndex ? 5 : 4,
        lostSales: policyIndex ? 0 : 1,
        orders: 2,
        purchaseOrders: 1,
        partialReceipts: 0,
        missedDeliveries: 0,
        constraintBlockedPurchases: 0,
        reserveBlockedPurchases: 0,
        deferredPurchaseReviews: 0,
        roundedPurchaseOrders: 0,
        extraPurchasedUnits: 0,
        revenue: '12.50',
        lostRevenue: '2.50',
        purchasingSpend: '5.00',
        endingCash: addDecimals(cash[index], policyIndex ? deltas[index] : '0'),
        minimumCash: policyIndex ? '1.000001' : '1.00',
        grossMargin: '6.25',
        endingInventoryValue: '1.25',
        inTransitInventoryValue: '0.00',
      },
      inventory: [],
      timeline: index === 1 ? [{ type: 'purchase_budget_blocked' }] : [],
      reconciliation: {
        stockBalanced: true,
        cashBalanced: true,
        cashFlowBalanced: true,
        inventoryFlowBalanced: true,
        inventoryValueBalanced: true,
        purchaseOrdersBalanced: true,
        purchasingConstraintsBalanced: true,
      },
    })),
  );
}

test('empirical percentiles and paired deltas preserve exact money beyond safe integer range', () => {
  const result = summarizeInventoryExperiment(experiment, reports().reverse());
  assert.equal(result.nativeRuns, 6);
  assert.equal(result.sameDemandPaths, true);
  assert.equal(result.sameHorizon, true);
  assert.equal(result.sameSupplyPaths, false);
  const [baseline, alternative] = result.policies;
  assert.equal(baseline.metrics.endingCash.p50, '9007199254740993.02');
  assert.deepEqual(baseline.metrics.endingCash.mean, {
    numerator: '27021597764222979.08',
    denominator: 3,
  });
  assert.equal(baseline.metrics.endingCash.p10, '9007199254740993.01');
  assert.equal(baseline.metrics.endingCash.p90, '9007199254740993.05');
  assert.equal(baseline.stockoutRuns, 3);
  assert.equal(alternative.stockoutRuns, 0);
  assert.equal(alternative.budgetBlockedRuns, 1);
  assert.deepEqual(alternative.pairedDeltaFromBaseline.metrics.endingCash, {
    min: '-0.02',
    p10: '-0.02',
    p50: '0.01',
    p90: '0.03',
    max: '0.03',
    mean: { numerator: '0.02', denominator: 3 },
  });
  assert.deepEqual(alternative.pairedDeltaFromBaseline.signs.endingCash, {
    positive: 2,
    zero: 0,
    negative: 1,
  });
  assert.equal(alternative.pairedDeltaFromBaseline.metrics.lostSales.p50, -1);
  assert.equal(alternative.pairedDeltaFromBaseline.metrics.minimumCash.p50, '0.000001');
  assert.deepEqual(
    baseline.runs.map((run) => run.seed),
    experiment.seeds,
  );
  assert.equal(baseline.runs[0].demandHash, alternative.runs[0].demandHash);
});

test('experiment aggregation fails closed on missing, repeated or unreconciled native reports', () => {
  const complete = reports();
  assert.throws(() => summarizeInventoryExperiment(experiment, complete.slice(1)), /exactly one/);
  assert.throws(
    () => summarizeInventoryExperiment(experiment, [...complete.slice(1), complete[1]]),
    /exactly one/,
  );
  const broken = reports();
  broken[0].reconciliation.cashBalanced = false;
  assert.throws(
    () => summarizeInventoryExperiment(experiment, broken),
    /identity or reconciliation/,
  );
  const mismatched = reports();
  mismatched[0].currency = 'EUR';
  assert.throws(
    () => summarizeInventoryExperiment(experiment, mismatched),
    /identity or reconciliation/,
  );
  const uncheckedPurchases = reports();
  uncheckedPurchases[0].reconciliation.purchaseOrdersBalanced = false;
  assert.throws(
    () => summarizeInventoryExperiment(experiment, uncheckedPurchases),
    /identity or reconciliation/,
  );
  const uncheckedConstraints = reports();
  uncheckedConstraints[0].reconciliation.purchasingConstraintsBalanced = false;
  assert.throws(
    () => summarizeInventoryExperiment(experiment, uncheckedConstraints),
    /identity or reconciliation/,
  );
  const uncheckedCashFlow = reports();
  uncheckedCashFlow[0].reconciliation.cashFlowBalanced = false;
  assert.throws(
    () => summarizeInventoryExperiment(experiment, uncheckedCashFlow),
    /identity or reconciliation/,
  );
  const uncheckedInventoryFlow = reports();
  uncheckedInventoryFlow[0].reconciliation.inventoryFlowBalanced = false;
  assert.throws(
    () => summarizeInventoryExperiment(experiment, uncheckedInventoryFlow),
    /identity or reconciliation/,
  );
});

test('experiment comparisons expose changed demand paths and different horizons', () => {
  const changed = reports();
  changed[3].demandForecast.products[0].dailyUnits[0] = 123;
  assert.equal(summarizeInventoryExperiment(experiment, changed).sameDemandPaths, false);
  const differentHorizon = { ...experiment, plans: [policy, { ...experiment.plans[1], days: 3 }] };
  for (const report of changed) if (report.name === 'alternative') report.days = 3;
  assert.equal(summarizeInventoryExperiment(differentHorizon, changed).sameHorizon, false);
});

test('experiment reports supply disruptions and matching supplier paths for pure policy changes', () => {
  const matched = {
    ...experiment,
    plans: [policy, { ...experiment.plans[1], products: [{ ...product, targetStock: 12 }] }],
  };
  const runs = reports();
  for (const run of runs)
    run.supplyProjection = projectSupply({
      ...matched.plans.find((plan) => plan.name === run.name),
      seed: run.seed,
    });
  runs[3].summary.partialReceipts = 1;
  runs[4].summary.missedDeliveries = 1;
  runs[3].summary.constraintBlockedPurchases = 2;
  runs[4].summary.reserveBlockedPurchases = 3;
  runs[4].summary.deferredPurchaseReviews = 2;
  runs[4].summary.extraPurchasedUnits = 7;
  const result = summarizeInventoryExperiment(matched, runs);
  assert.equal(result.sameSupplyPaths, true);
  assert.equal(result.policies[1].disruptedSupplyRuns, 2);
  assert.equal(result.policies[1].constraintBlockedRuns, 1);
  assert.equal(result.policies[1].reserveBlockedRuns, 1);
  assert.equal(result.policies[1].reviewDeferredRuns, 1);
  assert.equal(result.policies[1].pairedDeltaFromBaseline.metrics.deferredPurchaseReviews.max, 2);
  assert.equal(result.policies[1].metrics.reserveBlockedPurchases.max, 3);
  assert.equal(result.policies[1].pairedDeltaFromBaseline.metrics.extraPurchasedUnits.max, 7);
  assert.equal(result.policies[1].pairedDeltaFromBaseline.metrics.partialReceipts.max, 1);
  assert.equal(result.policies[0].runs[0].supplyHash, result.policies[1].runs[0].supplyHash);
});

test('experiment reports repeated demand and supply paths without reducing supplied sample weight', () => {
  const repeated = reports();
  for (const run of repeated) run.demandForecast.products[0].dailyUnits = [2, 3];
  const result = summarizeInventoryExperiment(experiment, repeated);
  for (const policy of result.policies) {
    assert.equal(policy.samples, 3);
    assert.deepEqual(policy.pathDiversity, {
      demandPaths: 1,
      supplyPaths: 1,
      jointPaths: 1,
      duplicateJointPathRuns: 2,
    });
    assert.equal(policy.metrics.endingCash.mean.denominator, 3);
    assert.equal(policy.runs.length, 3);
  }
});

test('path diversity counts paired trajectories independently of identical financial outcomes', () => {
  const paths = reports();
  const baseline = paths.slice(0, 3);
  baseline[0].demandForecast.products[0].dailyUnits = [1, 2];
  baseline[1].demandForecast.products[0].dailyUnits = [1, 2];
  baseline[2].demandForecast.products[0].dailyUnits = [2, 1];
  baseline[1].supplyProjection.products[0].dailyLeadTimeDays[0] = 2;
  baseline[2].supplyProjection = structuredClone(baseline[1].supplyProjection);
  for (const run of baseline) run.summary = { ...baseline[0].summary };
  const result = summarizeInventoryExperiment(experiment, paths).policies[0];
  assert.deepEqual(result.pathDiversity, {
    demandPaths: 2,
    supplyPaths: 2,
    jointPaths: 3,
    duplicateJointPathRuns: 0,
  });
  assert.equal(result.metrics.endingCash.min, result.metrics.endingCash.max);
});

test('experiment goals reject empty, mistyped, unbounded or unknown thresholds', () => {
  const goals = {
    minFillRateBps: 9500,
    minEndingCash: '10.000001',
    minCashBalance: '1.000001',
    minGrossMargin: '-2.50',
    maxPurchasingSpend: '20',
  };
  assert.deepEqual(inventoryExperimentSchema.parse({ ...experiment, goals }).goals, goals);
  for (const invalid of [
    {},
    { minFillRateBps: -1 },
    { minFillRateBps: 10001 },
    { minFillRateBps: 99.5 },
    { minFillRateBps: '9500' },
    { minEndingCash: 10 },
    { minEndingCash: '-1' },
    { minCashBalance: '-0.000001' },
    { minCashBalance: 1 },
    { minCashBalance: '0.0000001' },
    { maxPurchasingSpend: '1e3' },
    { minGrossMargin: '0.0000001' },
    { minGrossMargin: '-1000000000000000000' },
    { minEndingCash: '1', apply: true },
  ])
    assert.equal(
      inventoryExperimentSchema.safeParse({ ...experiment, goals: invalid }).success,
      false,
      JSON.stringify(invalid),
    );
});

test('fill rate pools units rather than averaging percentages and preserves exact boundaries', () => {
  const first = { demanded: 1, sold: 1, lostSales: 0 };
  const second = { demanded: 9, sold: 0, lostSales: 9 };
  assert.deepEqual(serviceLevel([first, second]), {
    demanded: 10,
    sold: 1,
    lostSales: 9,
    fillRate: { numerator: 1, denominator: 10 },
  });
  const third = { demanded: 3, sold: 1, lostSales: 2 };
  assert.equal(evaluateInventoryGoals(third, { minFillRateBps: 3333 }).status, 'met');
  assert.equal(evaluateInventoryGoals(third, { minFillRateBps: 3334 }).status, 'missed');
  assert.equal(evaluateInventoryGoals(first, { minFillRateBps: 10000 }).status, 'met');
  assert.throws(() => serviceLevel([{ ...first, lostSales: 1 }]), /balanced/);
  assert.throws(() => serviceLevel([{ demanded: 1.5, sold: 1, lostSales: 0.5 }]), /integer/);
});

test('per-product goals strictly validate bounded thresholds and unique SKUs present in every policy', () => {
  const goals = { productFillRates: [{ sku: 'SIM', minFillRateBps: 10000 }] };
  assert.deepEqual(inventoryExperimentSchema.parse({ ...experiment, goals }).goals, goals);
  for (const productFillRates of [
    [],
    [{ sku: 'UNKNOWN', minFillRateBps: 10000 }],
    [{ sku: 'SIM', minFillRateBps: -1 }],
    [{ sku: 'SIM', minFillRateBps: 10001 }],
    [{ sku: 'SIM', minFillRateBps: 99.5 }],
    [{ sku: 'SIM', minFillRateBps: '9500' }],
    [{ sku: 'SIM' }],
    [{ sku: 'SIM', minFillRateBps: 0, apply: true }],
    [
      { sku: 'SIM', minFillRateBps: 0 },
      { sku: 'SIM', minFillRateBps: 1 },
    ],
    Array.from({ length: 9 }, (_, index) => ({ sku: `SKU-${index}`, minFillRateBps: 0 })),
  ])
    assert.equal(
      inventoryExperimentSchema.safeParse({ ...experiment, goals: { productFillRates } }).success,
      false,
      JSON.stringify(productFillRates),
    );
  const missing = {
    ...experiment,
    plans: [policy, { ...policy, name: 'missing', products: [{ ...product, sku: 'OTHER' }] }],
    goals,
  };
  const rejected = inventoryExperimentSchema.safeParse(missing);
  assert.equal(rejected.success, false);
  assert.ok(rejected.error.issues.some((issue) => issue.message.includes('every policy')));
  const contract = getSimulationKernelContract();
  const target = contract.schemas.inventoryExperiment.properties.goals.properties.productFillRates;
  assert.equal(target.minItems, 1);
  assert.equal(target.maxItems, 8);
  assert.equal(target.items.properties.minFillRateBps.maximum, 10000);
});

test('per-product fill checks use exact boundaries and refuse missing, duplicate or unbalanced inventory evidence', () => {
  const summary = { demanded: 100, sold: 98, lostSales: 2 };
  const inventory = [
    { sku: 'SIM', demanded: 3, sold: 1, lostSales: 2 },
    { sku: 'OTHER', demanded: 97, sold: 97, lostSales: 0 },
  ];
  const goals = { minFillRateBps: 9800, productFillRates: [{ sku: 'SIM', minFillRateBps: 3334 }] };
  const result = evaluateInventoryGoals(summary, goals, inventory);
  assert.equal(result.checks.minFillRateBps.status, 'met');
  assert.equal(result.status, 'missed');
  assert.deepEqual(result.checks.productFillRates.products, [
    {
      sku: 'SIM',
      threshold: 3334,
      actual: { numerator: 1, denominator: 3 },
      status: 'missed',
    },
  ]);
  assert.equal(
    evaluateInventoryGoals(
      summary,
      {
        productFillRates: [{ sku: 'SIM', minFillRateBps: 3333 }],
      },
      inventory,
    ).status,
    'met',
  );
  assert.throws(() => evaluateInventoryGoals(summary, goals), /exactly one/);
  assert.throws(
    () => evaluateInventoryGoals(summary, goals, [inventory[0], inventory[0]]),
    /exactly one/,
  );
  assert.throws(
    () => evaluateInventoryGoals(summary, goals, [{ ...inventory[0], lostSales: 0 }]),
    /balanced/,
  );
});

test('per-product goals require joint success on each seed and pool each SKU independently', () => {
  const input = {
    ...experiment,
    plans: [{ ...policy, products: [product, { ...product, sku: 'SECOND' }] }],
    goals: {
      minFillRateBps: 8000,
      productFillRates: [
        { sku: 'SIM', minFillRateBps: 10000 },
        { sku: 'SECOND', minFillRateBps: 10000 },
      ],
    },
  };
  const runs = reports().slice(0, 3);
  for (const [index, run] of runs.entries()) {
    const firstSold = index === 1 ? 0 : 1;
    const secondSold = index === 0 ? 3 : 4;
    run.inventory = [
      { sku: 'SECOND', demanded: 4, sold: secondSold, lostSales: 4 - secondSold },
      { sku: 'SIM', demanded: 1, sold: firstSold, lostSales: 1 - firstSold },
    ];
    run.summary = {
      ...run.summary,
      sold: firstSold + secondSold,
      lostSales: 5 - firstSold - secondSold,
    };
  }
  const result = summarizeInventoryExperiment(input, runs.reverse()).policies[0];
  assert.equal(result.evaluation.metRuns, 1);
  assert.equal(result.evaluation.missedRuns, 2);
  assert.equal(result.evaluation.byGoal.minFillRateBps.metRuns, 3);
  assert.equal(result.evaluation.byGoal.productFillRates.metRuns, 1);
  assert.deepEqual(
    result.evaluation.byGoal.productFillRates.products.map((item) => [
      item.sku,
      item.metRuns,
      item.missedRuns,
      item.serviceLevel.fillRate,
    ]),
    [
      ['SIM', 2, 1, { numerator: 2, denominator: 3 }],
      ['SECOND', 2, 1, { numerator: 11, denominator: 12 }],
    ],
  );
  runs[0].inventory.find((item) => item.sku === 'SIM').demanded = 9;
  runs[0].inventory.find((item) => item.sku === 'SIM').lostSales = 8;
  const pooled = summarizeInventoryExperiment(input, runs).policies[0].evaluation.byGoal
    .productFillRates.products[0];
  assert.deepEqual(pooled.serviceLevel.fillRate, { numerator: 2, denominator: 11 });
});

test('zero-demand product goals stay unevaluable and missed goals take precedence', () => {
  const summary = { demanded: 1, sold: 0, lostSales: 1 };
  const inventory = [
    { sku: 'IDLE', demanded: 0, sold: 0, lostSales: 0 },
    { sku: 'SIM', ...summary },
  ];
  const goals = { productFillRates: [{ sku: 'IDLE', minFillRateBps: 0 }] };
  const idle = evaluateInventoryGoals(summary, goals, inventory);
  assert.equal(idle.status, 'not-evaluable');
  assert.equal(idle.checks.productFillRates.products[0].actual, null);
  assert.equal(
    evaluateInventoryGoals(summary, { ...goals, minFillRateBps: 1 }, inventory).status,
    'missed',
  );
  assert.equal(
    evaluateInventoryGoals(
      summary,
      {
        productFillRates: [...goals.productFillRates, { sku: 'SIM', minFillRateBps: 10000 }],
      },
      inventory,
    ).checks.productFillRates.status,
    'missed',
  );
  const input = {
    ...experiment,
    plans: [policy],
    goals: { productFillRates: [{ sku: 'SIM', minFillRateBps: 0 }] },
  };
  const runs = reports().slice(0, 3);
  for (const run of runs) run.inventory = [{ sku: 'SIM', demanded: 0, sold: 0, lostSales: 0 }];
  const result = summarizeInventoryExperiment(input, runs).policies[0].evaluation;
  assert.equal(result.notEvaluableRuns, 3);
  assert.equal(result.allRunsMet, false);
  assert.equal(result.byGoal.productFillRates.products[0].notEvaluableRuns, 3);
  assert.equal(result.byGoal.productFillRates.products[0].serviceLevel.fillRate, null);
});

test('goal evaluation preserves exact money, signed margins and inclusive thresholds', () => {
  const summary = {
    demanded: 4,
    sold: 3,
    lostSales: 1,
    endingCash: '9007199254740993.000001',
    minimumCash: '9007199254740993.000000',
    grossMargin: '-0.000001',
    purchasingSpend: '0.100001',
  };
  const goals = {
    minFillRateBps: 7500,
    minEndingCash: '9007199254740993.000001',
    minCashBalance: '9007199254740993.000000',
    minGrossMargin: '-0.000001',
    maxPurchasingSpend: '0.100001',
  };
  const result = evaluateInventoryGoals(summary, goals);
  assert.equal(result.status, 'met');
  assert.deepEqual(result.checks.minFillRateBps.actual, { numerator: 3, denominator: 4 });
  assert.equal(result.checks.minEndingCash.actual, summary.endingCash);
  assert.equal(result.checks.minCashBalance.actual, summary.minimumCash);
  assert.equal(
    evaluateInventoryGoals(summary, { minCashBalance: '9007199254740993.000001' }).status,
    'missed',
  );
  assert.equal(
    evaluateInventoryGoals(summary, { minEndingCash: '9007199254740993.000002' }).status,
    'missed',
  );
  assert.equal(
    evaluateInventoryGoals(summary, { maxPurchasingSpend: '0.100000' }).status,
    'missed',
  );
  assert.equal(evaluateInventoryGoals(summary, { minGrossMargin: '0' }).status, 'missed');
});

test('goals count jointly satisfied runs and keep no-demand service evidence undefined', () => {
  const input = {
    ...experiment,
    goals: { minFillRateBps: 9000, minEndingCash: '9007199254740993.02' },
  };
  const runs = reports();
  runs[5].summary = { ...runs[5].summary, sold: 4, lostSales: 1 };
  const result = summarizeInventoryExperiment(input, runs);
  const [baseline, alternative] = result.policies;
  assert.equal(baseline.evaluation.metRuns, 0);
  assert.equal(baseline.evaluation.missedRuns, 3);
  assert.equal(alternative.evaluation.metRuns, 1);
  assert.equal(alternative.evaluation.byGoal.minFillRateBps.metRuns, 2);
  assert.equal(alternative.evaluation.byGoal.minEndingCash.metRuns, 2);
  assert.equal(alternative.evaluation.allRunsMet, false);
  assert.equal(alternative.runs[0].evaluation.checks.minEndingCash.status, 'met');
  assert.equal(baseline.runs[0].evaluation.checks.minFillRateBps.status, 'missed');
  assert.equal(summarizeInventoryExperiment(experiment, runs).policies[0].evaluation, null);
  runs[0].summary = { ...runs[0].summary, demanded: 0, sold: 0, lostSales: 0 };
  const zero = summarizeInventoryExperiment({ ...experiment, goals: { minFillRateBps: 0 } }, runs)
    .policies[0];
  assert.equal(zero.runs[0].serviceLevel.fillRate, null);
  assert.equal(zero.runs[0].evaluation.status, 'not-evaluable');
  assert.equal(zero.evaluation.notEvaluableRuns, 1);
  assert.equal(zero.evaluation.allRunsMet, false);
  assert.equal(zero.serviceLevel.zeroDemandRuns, 1);
  assert.deepEqual(zero.serviceLevel.fillRate, { numerator: 8, denominator: 10 });
  const noDemand = { demanded: 0, sold: 0, lostSales: 0, endingCash: '0' };
  assert.equal(
    evaluateInventoryGoals(noDemand, { minFillRateBps: 0, minEndingCash: '1' }).status,
    'missed',
  );
  assert.equal(evaluateInventoryGoals(noDemand, { minEndingCash: '0' }).status, 'met');
});

test('comparison flags disclose capital, economics and cash-priority changes with normalized decimals', () => {
  const parsed = inventoryExperimentSchema.parse(experiment);
  const same = summarizeInventoryExperiment(experiment, reports());
  for (const field of [
    'sameOpeningCash',
    'sameCashReserve',
    'sameOpeningInventory',
    'sameUnitEconomics',
    'sameProductPriority',
  ])
    assert.equal(same[field], true);
  const second = {
    ...parsed.plans[1],
    startingCash: '20.000',
    cashReserve: '0.000000',
    products: [{ ...product, unitCost: '1.25000' }],
  };
  const normalized = { ...experiment, plans: [parsed.plans[0], second] };
  assert.equal(summarizeInventoryExperiment(normalized, reports()).sameOpeningCash, true);
  assert.equal(summarizeInventoryExperiment(normalized, reports()).sameCashReserve, true);
  assert.equal(summarizeInventoryExperiment(normalized, reports()).sameUnitEconomics, true);
  const changed = {
    ...normalized,
    plans: [
      parsed.plans[0],
      {
        ...second,
        startingCash: '21',
        cashReserve: '1.00',
        products: [{ ...product, initialStock: 3, unitPrice: '2.51' }],
      },
    ],
  };
  const comparison = summarizeInventoryExperiment(changed, reports());
  assert.equal(comparison.sameOpeningCash, false);
  assert.equal(comparison.sameCashReserve, false);
  assert.equal(comparison.sameOpeningInventory, false);
  assert.equal(comparison.sameUnitEconomics, false);
  const twoProducts = [product, { ...product, sku: 'SECOND' }];
  const reordered = {
    ...experiment,
    plans: [
      { ...policy, products: twoProducts },
      { ...policy, name: 'alternative', products: [...twoProducts].reverse() },
    ],
  };
  const reorderedResult = summarizeInventoryExperiment(reordered, reports());
  assert.equal(reorderedResult.sameProductPriority, false);
  assert.equal(reorderedResult.sameOpeningInventory, true);
  assert.equal(reorderedResult.sameUnitEconomics, true);
});

test('experiment diagnostics remain paired by policy and seed when only one alternative changes conditions', () => {
  const parsed = inventoryExperimentSchema.parse(experiment);
  const third = { ...parsed.plans[0], name: 'third-policy', startingCash: '20.000001' };
  const input = { ...experiment, plans: [parsed.plans[0], parsed.plans[1], third] };
  const complete = reports();
  const thirdRuns = structuredClone(complete.slice(0, 3)).map((report) => ({
    ...report,
    name: third.name,
  }));
  thirdRuns[2].demandForecast.products[0].dailyUnits[0] += 1;
  thirdRuns[1].supplyProjection.products[0].dailyLeadTimeDays[0] += 1;
  const result = summarizeInventoryExperiment(input, [...complete, ...thirdRuns].reverse());
  const [baseline, alternative, changed] = result.policies;
  assert.equal(result.sameOpeningCash, false);
  assert.equal(result.sameDemandPaths, false);
  assert.equal(result.sameSupplyPaths, false);
  assert.deepEqual(baseline.comparisonToBaseline.changedConditions, []);
  assert.deepEqual(alternative.comparisonToBaseline.changedConditions, ['supplyPath']);
  assert.equal(alternative.comparisonToBaseline.sameOpeningCash, true);
  assert.equal(alternative.comparisonToBaseline.sameDemandPaths, true);
  assert.deepEqual(changed.comparisonToBaseline.changedConditions, [
    'openingCash',
    'demandPath',
    'supplyPath',
  ]);
});
