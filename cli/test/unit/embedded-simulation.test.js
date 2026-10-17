import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createCommerce } from '../../src/commerce.js';
import {
  runCommerceSimulation,
  compareCommerceSimulations,
  runInventorySimulation,
  compareInventorySimulations,
  commerceScenarioSchema,
  inventoryPlanSchema,
  runInventoryExperiment,
} from '../../src/simulation/index.js';
import { simulationTools } from '../../src/tools/simulations.js';
import { normalizeToolResult } from '../../src/mcp/tool-result-contract.js';

const scenario = {
  name: 'retry-and-refund',
  inventory: [{ sku: 'SIM', unitPrice: '0.10', quantity: 3 }],
  steps: [
    { type: 'place_order', id: 'order', items: [{ sku: 'SIM', quantity: 2 }] },
    { type: 'capture_payment', id: 'payment', order: 'order' },
    { type: 'retry_payment', payment: 'payment' },
    { type: 'refund_payment', id: 'refund', payment: 'payment', amount: '0.10' },
    { type: 'retry_refund', refund: 'refund' },
    {
      type: 'place_order',
      id: 'too-large',
      items: [{ sku: 'SIM', quantity: 2 }],
      expect: 'rejected',
    },
  ],
};
const plan = {
  name: 'replenish',
  days: 3,
  seed: 42,
  startingCash: '20',
  products: [
    {
      sku: 'PLAN',
      initialStock: 2,
      unitPrice: '2.50',
      unitCost: '1.25',
      demand: { min: 2, max: 2 },
      reorderPoint: 2,
      targetStock: 6,
      leadTimeDays: 1,
    },
  ],
};

test('multi-path experiments run independent native policies on matching seeded demand', async () => {
  const { seed: _seed, ...policy } = plan;
  const input = {
    name: 'native-policy-experiment',
    seeds: [7, 42],
    goals: {
      minFillRateBps: 10000,
      minEndingCash: '15',
      minCashBalance: '15',
      minGrossMargin: '5',
      maxPurchasingSpend: '10',
    },
    plans: [
      { ...policy, days: 2 },
      { ...policy, name: 'slow', days: 2, products: [{ ...policy.products[0], leadTimeDays: 60 }] },
    ],
  };
  const result = await runInventoryExperiment(input);
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.mode, 'inventory-experiment');
  assert.equal(result.liveStoreRead, false);
  assert.equal(result.liveStoreModified, false);
  assert.equal(result.experiment.nativeRuns, 4);
  assert.equal(result.experiment.sameDemandPaths, true);
  assert.equal(result.experiment.sameHorizon, true);
  assert.equal(result.experiment.sameSupplyPaths, false);
  const [fast, slow] = result.experiment.policies;
  assert.deepEqual(fast.comparisonToBaseline.changedConditions, []);
  assert.deepEqual(slow.comparisonToBaseline.changedConditions, ['supplyPath']);
  assert.equal(fast.metrics.sold.min, 4);
  assert.equal(fast.stockoutRuns, 0);
  assert.equal(fast.evaluation.metRuns, 2);
  assert.equal(fast.evaluation.allRunsMet, true);
  assert.deepEqual(fast.serviceLevel.fillRate, { numerator: 8, denominator: 8 });
  assert.equal(slow.stockoutRuns, 2);
  assert.equal(slow.evaluation.missedRuns, 2);
  assert.deepEqual(slow.runs[0].evaluation.checks.minFillRateBps.actual, {
    numerator: 2,
    denominator: 4,
  });
  assert.equal(slow.pairedDeltaFromBaseline.metrics.lostSales.p50, 2);
  for (const policyResult of result.experiment.policies) {
    assert.equal(policyResult.runs.length, 2);
    assert.ok(policyResult.runs.every((run) => run.reconciliation.stockBalanced));
  }
  const replay = await runInventorySimulation({ ...input.plans[1], seed: 42 });
  assert.deepEqual(slow.runs[1].summary, replay.results[0].summary);
  assert.deepEqual((await runInventoryExperiment(input)).experiment, result.experiment);
});

test('native no-demand experiments report unevaluable fill-rate goals without failing execution', async () => {
  const { seed: _seed, ...policy } = plan;
  const result = await runInventoryExperiment({
    name: 'no-demand-evidence',
    seeds: [7, 42],
    goals: {
      minFillRateBps: 0,
      minEndingCash: '0',
      productFillRates: [{ sku: 'PLAN', minFillRateBps: 0 }],
    },
    plans: [
      { ...policy, days: 1, products: [{ ...policy.products[0], demand: { min: 0, max: 0 } }] },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const outcome = result.experiment.policies[0];
  assert.equal(outcome.serviceLevel.fillRate, null);
  assert.equal(outcome.serviceLevel.zeroDemandRuns, 2);
  assert.equal(outcome.evaluation.notEvaluableRuns, 2);
  assert.equal(outcome.evaluation.allRunsMet, false);
  assert.equal(outcome.evaluation.byGoal.productFillRates.products[0].notEvaluableRuns, 2);
  assert.equal(outcome.evaluation.byGoal.productFillRates.products[0].serviceLevel.fillRate, null);
  assert.ok(outcome.runs.every((run) => run.evaluation.checks.minEndingCash.status === 'met'));
});

test('per-SKU native service goals expose critical stockouts hidden by a passing overall fill rate', async () => {
  const bulk = {
    sku: 'BULK',
    initialStock: 99,
    unitPrice: '1.00',
    unitCost: '1.00',
    demand: { dailyUnits: [99] },
    reorderPoint: 0,
    targetStock: 1,
    leadTimeDays: 1,
  };
  const critical = { ...bulk, sku: 'CRITICAL', initialStock: 0, demand: { dailyUnits: [1] } };
  const fast = { name: 'on-time', days: 1, startingCash: '1.00', products: [bulk, critical] };
  const input = {
    name: 'critical-service',
    seeds: [7, 42],
    goals: { minFillRateBps: 9900, productFillRates: [{ sku: 'CRITICAL', minFillRateBps: 10000 }] },
    plans: [fast, { ...fast, name: 'delayed', products: [bulk, { ...critical, leadTimeDays: 2 }] }],
  };
  const result = await runInventoryExperiment(input);
  assert.equal(result.success, true, JSON.stringify(result));
  const [onTime, delayed] = result.experiment.policies;
  assert.equal(onTime.evaluation.metRuns, 2);
  assert.equal(delayed.evaluation.byGoal.minFillRateBps.metRuns, 2);
  assert.equal(delayed.evaluation.missedRuns, 2);
  assert.deepEqual(delayed.evaluation.byGoal.productFillRates.products, [
    {
      sku: 'CRITICAL',
      threshold: 10000,
      metRuns: 0,
      missedRuns: 2,
      notEvaluableRuns: 0,
      serviceLevel: {
        demanded: 2,
        sold: 0,
        lostSales: 2,
        fillRate: { numerator: 0, denominator: 2 },
      },
    },
  ]);
  assert.deepEqual(delayed.serviceLevel.fillRate, { numerator: 198, denominator: 200 });
  assert.deepEqual(delayed.comparisonToBaseline.changedConditions, ['supplyPath']);
  assert.equal(result.liveStoreModified, false);
  for (const policy of result.experiment.policies) {
    for (const run of policy.runs) {
      assert.equal(run.reconciliation.stockBalanced, true);
      assert.equal(run.reconciliation.purchaseOrdersBalanced, true);
      assert.equal(run.reconciliation.cashFlowBalanced, true);
    }
  }
});

test('periodic native purchasing reviews defer orders and expose resulting stockouts on matching paths', async () => {
  const input = JSON.parse(
    await fs.readFile(
      new URL('../../examples/inventory-review-interval-experiment.json', import.meta.url),
      'utf8',
    ),
  );
  const result = await runInventoryExperiment(input);
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.experiment.sameDemandPaths, true);
  assert.equal(result.experiment.sameSupplyPaths, true);
  const [daily, periodic] = result.experiment.policies;
  assert.deepEqual(periodic.comparisonToBaseline.changedConditions, []);
  assert.equal(daily.metrics.lostSales.p50, 0);
  assert.equal(periodic.metrics.lostSales.p50, 1);
  assert.equal(daily.metrics.endingCash.p50, '17.00');
  assert.equal(periodic.metrics.endingCash.p50, '16.00');
  assert.equal(periodic.metrics.purchasingSpend.p50, '3.00');
  assert.equal(periodic.metrics.purchaseOrders.p50, 2);
  assert.equal(periodic.metrics.deferredPurchaseReviews.p50, 3);
  assert.equal(daily.reviewDeferredRuns, 0);
  assert.equal(periodic.reviewDeferredRuns, 2);
  assert.equal(periodic.evaluation.missedRuns, 2);
  assert.equal(periodic.pairedDeltaFromBaseline.metrics.deferredPurchaseReviews.p50, 3);
  const replay = await runInventorySimulation({ ...input.plans[1], seed: 99 });
  assert.equal(replay.success, true, JSON.stringify(replay));
  const report = replay.results[0];
  assert.deepEqual(report.summary, periodic.runs[0].summary);
  assert.deepEqual(
    report.timeline
      .filter((event) => event.type === 'purchase_order_sent')
      .map((event) => event.day),
    [0, 3],
  );
  assert.deepEqual(
    report.timeline
      .filter((event) => event.type === 'purchase_review_deferred')
      .map((event) => [event.day, event.inventoryPosition, event.nextReviewDay]),
    [
      [1, 1, 3],
      [2, 0, 3],
      [4, 1, 6],
    ],
  );
  assert.deepEqual(
    report.purchases.map((purchase) => [
      purchase.quantityOrdered,
      purchase.quantityReceived,
      purchase.status,
    ]),
    [
      [1, 1, 'received'],
      [2, 2, 'received'],
    ],
  );
  assert.equal(report.inventoryFlow.firstStockout.day, 3);
  assert.equal(report.inventoryFlow.daily[1].products[0].orderedUnits, 0);
  assert.equal(report.inventoryFlow.daily[3].products[0].orderedUnits, 2);
  assert.equal(report.reconciliation.inventoryFlowBalanced, true);
  assert.equal(report.reconciliation.cashFlowBalanced, true);
  assert.equal(report.reconciliation.purchaseOrdersBalanced, true);
});

test('native prepaid purchases expose a temporary liquidity trough and miss cash-reserve goals independently of ending cash', async () => {
  const result = await runInventoryExperiment({
    name: 'prepaid-liquidity',
    seeds: [7, 42],
    goals: { minEndingCash: '4.00', minCashBalance: '0.000001' },
    plans: [
      {
        name: 'fully-prepaid',
        days: 2,
        startingCash: '2.00',
        products: [
          {
            sku: 'CASH',
            initialStock: 0,
            unitPrice: '3.00',
            unitCost: '1.00',
            demand: { dailyUnits: [2, 0] },
            reorderPoint: 0,
            targetStock: 2,
            leadTimeDays: 1,
          },
        ],
      },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const policy = result.experiment.policies[0];
  assert.equal(policy.metrics.minimumCash.p50, '0.00');
  assert.equal(policy.metrics.endingCash.p50, '4.00');
  assert.equal(policy.evaluation.byGoal.minEndingCash.metRuns, 2);
  assert.equal(policy.evaluation.byGoal.minCashBalance.missedRuns, 2);
  for (const run of policy.runs) {
    assert.equal(run.reconciliation.cashFlowBalanced, true);
    assert.equal(run.cashFlow.minimumCash, run.summary.minimumCash);
    assert.deepEqual(run.cashFlow.minimumCashAt, {
      day: 0,
      type: 'purchase_order_sent',
      sku: 'CASH',
      purchase: 'purchase-1',
    });
    assert.deepEqual(
      run.cashFlow.daily.map((day) => day.closingCash),
      ['0.00', '4.00', '4.00'],
    );
    assert.equal(run.evaluation.checks.minCashBalance.actual, '0.00');
  }
});

test('cash reserves block native purchases without spending and sales later release the original order policy', async () => {
  const baseline = {
    name: 'unreserved',
    days: 2,
    startingCash: '2.00',
    products: [
      {
        sku: 'RESERVE',
        initialStock: 1,
        unitPrice: '3.00',
        unitCost: '1.00',
        demand: { dailyUnits: [1, 1] },
        reorderPoint: 1,
        targetStock: 3,
        leadTimeDays: 1,
      },
    ],
  };
  const result = await runInventoryExperiment({
    name: 'cash-reserve-policy',
    seeds: [7, 42],
    goals: { minCashBalance: '1.00' },
    plans: [baseline, { ...baseline, name: 'reserved', cashReserve: '1.00' }],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const [open, reserved] = result.experiment.policies;
  assert.equal(result.experiment.sameCashReserve, false);
  assert.equal(result.experiment.sameOpeningCash, true);
  assert.equal(result.experiment.sameDemandPaths, true);
  assert.equal(result.experiment.sameSupplyPaths, true);
  assert.deepEqual(reserved.comparisonToBaseline.changedConditions, ['cashReserve']);
  assert.equal(open.metrics.minimumCash.p50, '0.00');
  assert.equal(open.evaluation.missedRuns, 2);
  assert.equal(reserved.metrics.minimumCash.p50, '2.00');
  assert.equal(reserved.metrics.endingCash.p50, '5.00');
  assert.equal(reserved.reserveBlockedRuns, 2);
  assert.equal(reserved.metrics.reserveBlockedPurchases.p50, 1);
  assert.equal(reserved.metrics.purchaseOrders.p50, 1);
  assert.equal(reserved.metrics.lostSales.p50, 0);
  assert.equal(reserved.evaluation.metRuns, 2);
  const replay = await runInventorySimulation({
    ...baseline,
    name: 'reserved',
    seed: 42,
    cashReserve: '1.00',
  });
  const report = replay.results[0];
  assert.deepEqual(report.summary, reserved.runs[1].summary);
  assert.equal(report.reconciliation.cashFlowBalanced, true);
  assert.equal(report.assumptions.cashReserve, '1.00');
  assert.equal(report.purchases.length, 1);
  assert.equal(report.purchases[0].quantityOrdered, 3);
  assert.equal(report.purchases[0].quantityReceived, 3);
  assert.deepEqual(
    report.timeline
      .filter((event) => event.type === 'purchase_order_sent')
      .map((event) => event.day),
    [1],
  );
  assert.deepEqual(
    report.timeline.find((event) => event.type === 'purchase_reserve_blocked'),
    {
      day: 0,
      type: 'purchase_reserve_blocked',
      sku: 'RESERVE',
      quantity: 2,
      requestedQuantity: 2,
      extraUnits: 0,
      inventoryPosition: 1,
      minimumOrderQuantity: 1,
      orderMultiple: 1,
      required: '2.00',
      cash: '2.00',
      cashReserve: '1.00',
      spendableCash: '1.00',
    },
  );
});

test('multiple native products share one cash reserve and keep the supplied purchasing priority', async () => {
  const product = {
    initialStock: 0,
    unitPrice: '2.00',
    unitCost: '1.00',
    demand: { dailyUnits: [0] },
    reorderPoint: 0,
    targetStock: 2,
    leadTimeDays: 1,
  };
  const result = await runInventorySimulation({
    name: 'shared-reserve',
    days: 1,
    startingCash: '5.00',
    cashReserve: '2.00',
    products: [
      { ...product, sku: 'FIRST' },
      { ...product, sku: 'SECOND' },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const report = result.results[0];
  assert.equal(report.purchases.length, 1);
  assert.equal(report.purchases[0].sku, 'FIRST');
  assert.equal(report.summary.minimumCash, '3.00');
  assert.equal(report.summary.endingCash, '3.00');
  assert.equal(report.summary.purchasingSpend, '2.00');
  assert.equal(report.summary.reserveBlockedPurchases, 2);
  assert.deepEqual(
    report.timeline
      .filter((event) => event.type === 'purchase_reserve_blocked')
      .map((event) => [event.day, event.sku, event.spendableCash]),
    [
      [0, 'SECOND', '1.00'],
      [1, 'SECOND', '1.00'],
    ],
  );
  assert.equal(report.reconciliation.cashFlowBalanced, true);
});

test('explicit demand spikes drive native purchases, stockouts and goal checks on their supplied days', async () => {
  const dailyUnits = [0, 0, 8, 0];
  const product = {
    ...plan.products[0],
    demand: { dailyUnits },
    reorderPoint: 0,
    targetStock: 2,
    leadTimeDays: 2,
  };
  const lean = { name: 'lean-spike', days: 4, startingCash: '20.00', products: [product] };
  const replay = await runInventorySimulation({ ...lean, seed: 99 });
  assert.equal(replay.success, true, JSON.stringify(replay));
  const report = replay.results[0];
  assert.equal(report.assumptions.demand[0].model, 'explicit-daily-units');
  assert.deepEqual(report.assumptions.demand[0].input.dailyUnits, dailyUnits);
  assert.deepEqual(report.demandForecast.products[0].dailyUnits, dailyUnits);
  assert.deepEqual(
    report.timeline
      .filter((event) => event.type === 'demand')
      .map(({ day, demand, sold, lost }) => ({ day, demand, sold, lost })),
    [
      { day: 1, demand: 0, sold: 0, lost: 0 },
      { day: 2, demand: 0, sold: 0, lost: 0 },
      { day: 3, demand: 8, sold: 2, lost: 6 },
      { day: 4, demand: 0, sold: 0, lost: 0 },
    ],
  );
  assert.equal(report.purchases.length, 1);
  assert.equal(report.purchases[0].quantityOrdered, 2);
  assert.equal(report.purchases[0].quantityReceived, 0);
  assert.equal(report.purchases[0].status, 'sent');
  assert.equal(report.outstandingPurchases[0].arrivalDay, 5);
  assert.equal(report.summary.endingCash, '22.50');
  assert.equal(report.summary.inTransitInventoryValue, '2.50');
  const result = await runInventoryExperiment({
    name: 'forecast-spike-comparison',
    seeds: [7, 42],
    goals: { minFillRateBps: 9500, minGrossMargin: '10.00', maxPurchasingSpend: '20.00' },
    plans: [
      lean,
      {
        ...lean,
        name: 'buffered-spike',
        products: [{ ...product, reorderPoint: 2, targetStock: 10 }],
      },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.experiment.sameDemandPaths, true);
  assert.equal(result.experiment.sameSupplyPaths, true);
  const [limited, buffered] = result.experiment.policies;
  assert.equal(limited.metrics.lostSales.min, 6);
  assert.equal(limited.evaluation.missedRuns, 2);
  assert.equal(buffered.metrics.sold.min, 8);
  assert.equal(buffered.metrics.lostSales.max, 0);
  assert.equal(buffered.metrics.grossMargin.min, '10.00');
  assert.equal(buffered.evaluation.metRuns, 2);
  assert.deepEqual(buffered.pathDiversity, {
    demandPaths: 1,
    supplyPaths: 1,
    jointPaths: 1,
    duplicateJointPathRuns: 1,
  });
  assert.ok(
    buffered.runs.every(
      (run) =>
        run.reconciliation.stockBalanced &&
        run.reconciliation.cashBalanced &&
        run.reconciliation.purchaseOrdersBalanced,
    ),
  );
  assert.deepEqual(limited.runs[0].summary, report.summary);
});

test('explicit supplier shocks follow placement days through native receipts, lost sales and reconciliation', async () => {
  const product = {
    sku: 'CALENDAR',
    initialStock: 1,
    unitPrice: '3.00',
    unitCost: '1.00',
    demand: { dailyUnits: [1, 2, 1, 1] },
    reorderPoint: 1,
    targetStock: 3,
    leadTimeDays: 1,
  };
  const baseline = { name: 'normal-supply', days: 4, startingCash: '10.00', products: [product] };
  const shock = {
    ...baseline,
    name: 'scheduled-shock',
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
  };
  const result = await runInventoryExperiment({
    name: 'supplier-calendar',
    seeds: [7, 42],
    goals: { minFillRateBps: 10000 },
    plans: [baseline, shock],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const [normal, disrupted] = result.experiment.policies;
  assert.equal(result.experiment.sameDemandPaths, true);
  assert.equal(result.experiment.sameSupplyPaths, false);
  assert.deepEqual(disrupted.comparisonToBaseline.changedConditions, ['supplyPath']);
  assert.equal(normal.metrics.lostSales.p50, 0);
  assert.equal(disrupted.metrics.lostSales.p50, 2);
  assert.equal(normal.metrics.endingCash.p50, '18.00');
  assert.equal(disrupted.metrics.endingCash.p50, '15.00');
  assert.equal(disrupted.metrics.partialReceipts.p50, 1);
  assert.equal(normal.evaluation.metRuns, 2);
  assert.equal(disrupted.evaluation.missedRuns, 2);
  assert.equal(disrupted.pathDiversity.jointPaths, 1);
  assert.equal(disrupted.pathDiversity.duplicateJointPathRuns, 1);
  const replay = await runInventorySimulation({ ...shock, seed: 99 });
  assert.equal(replay.success, true, JSON.stringify(replay));
  const report = replay.results[0];
  assert.deepEqual(report.summary, disrupted.runs[0].summary);
  assert.deepEqual(
    report.timeline
      .filter((event) => event.type === 'purchase_order_sent')
      .map((event) => [event.day, event.quantity, event.arrivalDay, event.firstReceiptBps]),
    [
      [0, 2, 3, 5000],
      [3, 2, 4, 10000],
    ],
  );
  assert.deepEqual(
    report.purchases.map((purchase) => [
      purchase.quantityOrdered,
      purchase.quantityReceived,
      purchase.status,
    ]),
    [
      [2, 2, 'received'],
      [2, 2, 'received'],
    ],
  );
  assert.equal(report.inventory[0].onHand, 2);
  assert.equal(report.reconciliation.purchaseOrdersBalanced, true);
  assert.equal(report.reconciliation.cashFlowBalanced, true);
  assert.equal(report.reconciliation.inventoryValueBalanced, true);
  assert.equal(report.reconciliation.inventoryFlowBalanced, true);
  assert.deepEqual(report.inventoryFlow.firstStockout, {
    day: 2,
    sku: 'CALENDAR',
    demanded: 2,
    sold: 0,
    lostSales: 2,
  });
  assert.deepEqual(
    report.inventoryFlow.daily.map(({ day, products: [stock] }) => [
      day,
      stock.receivedUnits,
      stock.sold,
      stock.lostSales,
      stock.orderedUnits,
      stock.closingStock,
      stock.onOrder,
    ]),
    [
      [0, 0, 0, 0, 2, 1, 2],
      [1, 0, 1, 0, 0, 0, 2],
      [2, 0, 0, 2, 0, 0, 2],
      [3, 1, 1, 0, 2, 0, 3],
      [4, 3, 1, 0, 0, 2, 0],
    ],
  );
  assert.equal(report.supplyProjection.products[0].leadTimeMethod, 'explicit-daily-values');
});

test('split native receipts preserve prepaid in-transit value and receive the remaining balance later', async () => {
  const input = {
    name: 'partial-supplier-delivery',
    days: 2,
    seed: 42,
    startingCash: '1.00',
    products: [
      {
        sku: 'PARTIAL',
        initialStock: 0,
        unitPrice: '2.50',
        unitCost: '0.100001',
        demand: { min: 1, max: 1 },
        reorderPoint: 0,
        targetStock: 3,
        leadTimeDays: { min: 1, max: 1 },
        delivery: { firstReceiptBps: 5000, remainderDelayDays: 3 },
      },
    ],
  };
  const first = await runInventorySimulation(input);
  assert.equal(first.success, true, JSON.stringify(first));
  const [partial] = first.results;
  assert.equal(partial.summary.partialReceipts, 1);
  assert.equal(partial.summary.receiptEvents, 1);
  assert.equal(partial.summary.receivedPurchaseOrders, 0);
  assert.equal(partial.summary.sold, 1);
  assert.equal(partial.summary.lostSales, 1);
  assert.equal(partial.summary.purchasingSpend, '0.300003');
  assert.equal(partial.summary.endingCash, '3.199997');
  assert.equal(partial.summary.inTransitInventoryValue, '0.200002');
  assert.equal(partial.reconciliation.purchaseOrdersBalanced, true);
  assert.equal(partial.reconciliation.endingAssets, '3.399999');
  assert.deepEqual(partial.purchases, [
    {
      purchase: 'purchase-1',
      sku: 'PARTIAL',
      status: 'partially_received',
      currency: 'USD',
      total: '0.300003',
      quantityOrdered: 3,
      quantityReceived: 1,
      unitCost: '0.100001',
      lineTotal: '0.300003',
    },
  ]);
  assert.deepEqual(partial.outstandingPurchases, [
    {
      purchase: 'purchase-1',
      sku: 'PARTIAL',
      quantity: 2,
      arrivalDay: 4,
      orderedQuantity: 3,
      receivedQuantity: 1,
    },
  ]);
  const firstReceipt = partial.timeline.find((event) => event.type === 'purchase_received');
  assert.equal(firstReceipt.quantity, 1); // Floor the 1.5-unit first delivery.
  assert.equal(firstReceipt.complete, false);
  assert.equal(firstReceipt.nextArrivalDay, 4);
  const final = await runInventorySimulation({ ...input, days: 4 });
  assert.equal(final.success, true, JSON.stringify(final));
  const [completed] = final.results;
  assert.equal(completed.summary.receivedPurchaseOrders, 1);
  assert.equal(completed.summary.receiptEvents, 2);
  assert.equal(completed.summary.partialReceipts, 1);
  assert.equal(completed.summary.sold, 2);
  assert.equal(completed.summary.lostSales, 2);
  assert.equal(completed.summary.endingCash, '5.699997');
  assert.equal(completed.summary.endingInventoryValue, '0.100001');
  assert.equal(completed.summary.inTransitInventoryValue, '0.00');
  assert.equal(completed.inventory[0].receivedUnits, 3);
  assert.equal(completed.inventory[0].onHand, 1);
  assert.equal(completed.outstandingPurchases.length, 0);
  assert.equal(completed.reconciliation.purchaseOrdersBalanced, true);
  assert.equal(completed.purchases[0].status, 'received');
  assert.equal(completed.purchases[0].quantityReceived, 3);
});

test('a missed first delivery never creates a zero receipt or phantom stock', async () => {
  const result = await runInventorySimulation({
    name: 'missed-first-delivery',
    days: 2,
    startingCash: '1.00',
    products: [
      {
        sku: 'MISSED',
        initialStock: 0,
        unitPrice: '2.50',
        unitCost: '0.100001',
        demand: { min: 0, max: 0 },
        reorderPoint: 0,
        targetStock: 3,
        leadTimeDays: 1,
        delivery: { firstReceiptBps: 0, remainderDelayDays: 1 },
      },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const [run] = result.results;
  assert.equal(run.summary.missedDeliveries, 1);
  assert.equal(run.summary.partialReceipts, 0);
  assert.equal(run.summary.receiptEvents, 1);
  assert.equal(run.summary.receivedPurchaseOrders, 1);
  assert.equal(run.inventory[0].onHand, 3);
  assert.equal(run.inventory[0].receivedUnits, 3);
  assert.equal(run.summary.endingCash, '0.699997');
  assert.equal(run.summary.inTransitInventoryValue, '0.00');
  assert.equal(run.reconciliation.endingAssets, '1.00');
  assert.deepEqual(
    run.timeline
      .filter((event) => event.type.startsWith('purchase_'))
      .map((event) => [event.day, event.type]),
    [
      [0, 'purchase_order_sent'],
      [1, 'purchase_delivery_missed'],
      [2, 'purchase_received'],
    ],
  );
});

test('native purchases honor minimum quantities and case packs before spending or capacity checks', async () => {
  const input = {
    name: 'native-case-packs',
    days: 1,
    startingCash: '20.00',
    products: [
      {
        sku: 'PACK',
        initialStock: 0,
        unitPrice: '2.50',
        unitCost: '0.100001',
        demand: { min: 0, max: 0 },
        reorderPoint: 0,
        targetStock: 3,
        leadTimeDays: 1,
        purchasing: { minimumOrderQuantity: 5, orderMultiple: 4 },
      },
    ],
  };
  const successful = await runInventorySimulation(input);
  assert.equal(successful.success, true, JSON.stringify(successful));
  const [run] = successful.results;
  assert.equal(run.purchases[0].quantityOrdered, 8);
  assert.equal(run.purchases[0].total, '0.800008');
  assert.equal(run.inventory[0].onHand, 8);
  assert.equal(run.summary.endingCash, '19.199992');
  assert.equal(run.summary.extraPurchasedUnits, 5);
  assert.equal(run.summary.roundedPurchaseOrders, 1);
  assert.equal(run.reconciliation.purchasingConstraintsBalanced, true);
  assert.equal(run.reconciliation.endingAssets, '20.00');
  const sent = run.timeline.find((event) => event.type === 'purchase_order_sent');
  assert.equal(sent.requestedQuantity, 3);
  assert.equal(sent.quantity, 8);
  const budget = await runInventorySimulation({ ...input, startingCash: '0.50' });
  assert.equal(budget.success, true, JSON.stringify(budget));
  assert.equal(budget.results[0].purchases.length, 0);
  assert.equal(budget.results[0].summary.purchasingSpend, '0.00');
  assert.equal(budget.results[0].summary.extraPurchasedUnits, 0);
  assert.equal(budget.results[0].summary.endingCash, '0.50');
  assert.ok(
    budget.results[0].timeline.some(
      (event) =>
        event.type === 'purchase_budget_blocked' &&
        event.quantity === 8 &&
        event.required === '0.800008',
    ),
  );
  const capacity = await runInventorySimulation({
    ...input,
    products: [
      {
        ...input.products[0],
        purchasing: { ...input.products[0].purchasing, maximumInventoryPosition: 6 },
      },
    ],
  });
  assert.equal(capacity.success, true, JSON.stringify(capacity));
  const [blocked] = capacity.results;
  assert.equal(blocked.purchases.length, 0);
  assert.equal(blocked.summary.purchasingSpend, '0.00');
  assert.equal(blocked.summary.endingCash, '20.00');
  assert.equal(blocked.summary.constraintBlockedPurchases, 2);
  assert.equal(blocked.inventory[0].onHand, 0);
  assert.ok(blocked.timeline.every((event) => event.type !== 'purchase_order_sent'));
  assert.equal(blocked.reconciliation.purchasingConstraintsBalanced, true);
});

test('native sandbox executes exact captures, refunds, idempotency and oversell refusal', async () => {
  const result = await runCommerceSimulation(scenario);
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.liveStoreRead, false);
  assert.equal(result.liveStoreModified, false);
  assert.equal(result.externalServicesCalled, false);
  const [run] = result.results;
  assert.equal(run.expectationsMet, true, JSON.stringify(run.outcomes));
  assert.deepEqual(run.summary, {
    orders: 1,
    payments: 1,
    completedRefunds: 1,
    captured: '0.20',
    refunded: '0.10',
    netCaptured: '0.10',
    rejectedSteps: 1,
  });
  assert.equal(run.inventory[0].available, 1);
  assert.equal(run.inventory[0].allocated, 2);
  const repeat = await runCommerceSimulation(scenario);
  assert.equal(repeat.scenarioHash, result.scenarioHash);
  assert.deepEqual(repeat.results, result.results);
});

test('comparisons use separate native inventory and normalize expected refusals', async () => {
  const result = await compareCommerceSimulations([
    scenario,
    {
      ...scenario,
      name: 'more-stock',
      inventory: [{ ...scenario.inventory[0], quantity: 4 }],
      steps: scenario.steps.slice(0, 3),
    },
  ]);
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.results[0].summary.orders, 1);
  assert.equal(result.results[1].inventory[0].available, 2);
});

test('model episodes purchase, receive, inspect and ship through native engine state', async () => {
  const input = {
    name: 'procurement-to-fulfillment',
    inventory: [{ sku: 'SIM', quantity: 0, unitPrice: '2.50' }],
    steps: [
      {
        type: 'place_order',
        id: 'no-stock',
        items: [{ sku: 'SIM', quantity: 1 }],
        expect: 'rejected',
      },
      {
        type: 'purchase_inventory',
        id: 'supply',
        items: [{ sku: 'SIM', quantity: 3, unitCost: '1.250001' }],
      },
      { type: 'inspect_state' },
      { type: 'receive_purchase_order', purchase: 'supply' },
      { type: 'inspect_state' },
      { type: 'place_order', id: 'sale', items: [{ sku: 'SIM', quantity: 2 }] },
      { type: 'capture_payment', id: 'paid', order: 'sale' },
      { type: 'ship_order', order: 'sale' },
      { type: 'inspect_state' },
      { type: 'receive_purchase_order', purchase: 'supply', expect: 'rejected' },
      { type: 'cancel_order', order: 'sale', expect: 'rejected' },
      { type: 'refund_payment', id: 'refund', payment: 'paid', amount: '2.50' },
    ],
  };
  const result = await runCommerceSimulation(input);
  assert.equal(result.success, true, JSON.stringify(result));
  const [run] = result.results;
  assert.equal(run.expectationsMet, true, JSON.stringify(run.outcomes));
  const snapshots = run.outcomes
    .filter((outcome) => outcome.type === 'inspect_state')
    .map((outcome) => outcome.observation);
  assert.equal(snapshots[0].purchases[0].total, '3.750003');
  assert.equal(snapshots[0].purchases[0].items[0].received, 0);
  assert.equal(snapshots[0].inventory[0].available, 0);
  assert.equal(snapshots[1].purchases[0].status, 'received');
  assert.equal(snapshots[1].purchases[0].items[0].received, 3);
  assert.equal(snapshots[1].inventory[0].available, 3);
  assert.deepEqual(snapshots[2].inventory, [{ sku: 'SIM', onHand: 1, available: 1, allocated: 0 }]);
  assert.equal(snapshots[2].orders[0].status, 'shipped');
  assert.equal(snapshots[2].payments[0].status, 'completed');
  assert.equal(run.state.payments[0].refunded, '2.50');
  assert.equal(run.summary.netCaptured, '2.50');
  assert.deepEqual(run.inventory, snapshots[2].inventory);
  assert.deepEqual((await runCommerceSimulation(input)).results, result.results);
});

test('unexpected native refusal preserves observable state and continues later actions', async () => {
  const result = await runCommerceSimulation({
    name: 'inspect-after-refusal',
    inventory: [{ sku: 'SIM', quantity: 0, unitPrice: '1.00' }],
    steps: [
      { type: 'receive_purchase_order', purchase: 'missing' },
      { type: 'place_order', id: 'no-stock', items: [{ sku: 'SIM', quantity: 1 }] },
      { type: 'restock', sku: 'SIM', quantity: 2 },
      { type: 'inspect_state' },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.results[0].expectationsMet, false);
  assert.equal(result.results[0].summary.orders, 0);
  assert.equal(result.results[0].outcomes[3].observation.inventory[0].available, 2);
  assert.deepEqual(result.results[0].state.purchases, []);
});

test('planning runs real purchases, receipts, shipped sales and exact cash arithmetic', async () => {
  const result = await runInventorySimulation(plan);
  assert.equal(result.success, true, JSON.stringify(result));
  const [run] = result.results;
  assert.equal(run.summary.demanded, 6);
  assert.equal(run.summary.sold, 6);
  assert.equal(run.summary.lostSales, 0);
  assert.equal(run.summary.purchaseOrders, 2);
  assert.equal(run.summary.receivedPurchaseOrders, 2);
  assert.equal(run.summary.revenue, '15.00');
  assert.equal(run.summary.purchasingSpend, '10.00');
  assert.equal(run.summary.endingCash, '25.00');
  assert.equal(run.summary.endingInventoryValue, '5.00');
  assert.equal(run.summary.inTransitInventoryValue, '0.00');
  assert.equal(run.summary.costOfGoodsSold, '7.50');
  assert.equal(run.summary.grossMargin, '7.50');
  assert.equal(run.demandForecast.products[0].projectedUnits, 6);
  assert.equal(run.reconciliation.stockBalanced, true);
  assert.equal(run.reconciliation.endingAssets, '30.00');
  assert.equal(run.inventory[0].onHand, 4);
  assert.equal(run.inventory[0].allocated, 0);
  assert.equal(run.outstandingPurchases.length, 0);
  const repeat = await runInventorySimulation(plan);
  assert.deepEqual(repeat.results, result.results);
});

test('all daily sales fund purchasing before product-priority budget allocation', async () => {
  const result = await runInventorySimulation({
    name: 'end-of-day-purchasing',
    days: 1,
    startingCash: '0',
    products: [
      {
        sku: 'BUY-FIRST',
        initialStock: 0,
        unitPrice: '5.00',
        unitCost: '5.00',
        demand: { min: 0, max: 0 },
        reorderPoint: 0,
        targetStock: 1,
        leadTimeDays: 1,
      },
      {
        sku: 'SELL-SECOND',
        initialStock: 1,
        unitPrice: '10.00',
        unitCost: '1.00',
        demand: { min: 1, max: 1 },
        reorderPoint: 0,
        targetStock: 1,
        leadTimeDays: 1,
      },
    ],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const [run] = result.results;
  assert.equal(run.summary.endingCash, '4.00');
  assert.equal(run.summary.purchasingSpend, '6.00');
  assert.equal(run.summary.inTransitInventoryValue, '6.00');
  assert.equal(run.outstandingPurchases.length, 2);
  const daily = run.timeline.filter((event) => event.day === 1);
  assert.deepEqual(
    daily.map((event) => event.type),
    ['demand', 'demand', 'purchase_order_sent', 'purchase_order_sent'],
  );
  assert.equal(run.reconciliation.inventoryValueBalanced, true);
});

test('planning exposes budget-blocked purchases and lost sales without negative cash', async () => {
  const result = await runInventorySimulation({
    ...plan,
    startingCash: '0',
    products: [{ ...plan.products[0], initialStock: 0 }],
  });
  assert.equal(result.success, true, JSON.stringify(result));
  const [run] = result.results;
  assert.equal(run.summary.sold, 0);
  assert.equal(run.summary.lostSales, 6);
  assert.equal(run.summary.lostRevenue, '15.00');
  assert.equal(run.summary.purchaseOrders, 0);
  assert.equal(run.summary.endingCash, '0.00');
  assert.equal(run.inventory[0].stockoutDays, 3);
  assert.ok(run.timeline.some((entry) => entry.type === 'purchase_budget_blocked'));
});

test('same seeded historical demand supports reproducible policy comparisons', async () => {
  const base = {
    ...plan,
    days: 5,
    products: [{ ...plan.products[0], demand: { history: [0, 1, 3, 5] } }],
  };
  const slow = { ...base, name: 'slow', products: [{ ...base.products[0], leadTimeDays: 60 }] };
  const richer = {
    ...base,
    name: 'more-capital-and-price',
    startingCash: '30.00',
    products: [{ ...base.products[0], initialStock: 3, unitPrice: '3.00' }],
  };
  const result = await compareInventorySimulations([base, slow, richer]);
  assert.equal(result.success, true, JSON.stringify(result));
  const demands = result.results.map((run) =>
    run.timeline.filter((entry) => entry.type === 'demand').map((entry) => entry.demand),
  );
  assert.deepEqual(demands[0], demands[1]);
  assert.deepEqual(demands[0], demands[2]);
  assert.equal(result.comparison.sameDemandPath, true);
  assert.equal(result.comparison.sameHorizon, true);
  assert.equal(result.comparison.baseline, base.name);
  assert.equal(result.comparison.sameOpeningCash, false);
  assert.equal(result.comparison.sameOpeningInventory, false);
  assert.equal(result.comparison.sameUnitEconomics, false);
  assert.deepEqual(result.comparison.rows[0].comparisonToBaseline.changedConditions, []);
  assert.deepEqual(result.comparison.rows[1].comparisonToBaseline.changedConditions, [
    'supplyPath',
  ]);
  assert.deepEqual(result.comparison.rows[2].comparisonToBaseline.changedConditions, [
    'openingCash',
    'openingInventory',
    'unitEconomics',
  ]);
  assert.equal(result.comparison.rows[0].deltaFromBaseline.endingCash, '0.00');
  assert.ok(result.results[1].outstandingPurchases.length > 0);
});

test('MCP simulation never accesses the supplied live commerce handle or operator database', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-simulation-sentinel-'));
  const file = path.join(directory, 'operator.db');
  const originalDatabase = process.env.STATESET_DB_PATH;
  try {
    // close() invalidates handles while already-started native work finishes.
    // Wait for fixture process exit so its final WAL checkpoint cannot be
    // mistaken for a simulation write. Do not rely on a timing-based sleep.
    await promisify(execFile)(
      process.execPath,
      [
        '--input-type=module',
        '--eval',
        `const { createCommerce } = await import(process.argv[1]);
       const commerce = createCommerce(process.argv[2]);
       await commerce.customers.create({ email: 'operator@example.test', firstName: 'Operator', lastName: 'Store' });
       await commerce.close();`,
        new URL('../../src/commerce.js', import.meta.url).href,
        file,
      ],
      { env: { NODE_ENV: 'test' } },
    );
    const digest = async () =>
      JSON.stringify(
        await Promise.all(
          (await fs.readdir(directory)).sort().map(async (name) => [
            name,
            createHash('sha256')
              .update(await fs.readFile(path.join(directory, name)))
              .digest('hex'),
          ]),
        ),
      );
    const before = await digest();
    process.env.STATESET_DB_PATH = file;
    const live = new Proxy(
      {},
      {
        get() {
          throw new Error('Live store was accessed');
        },
      },
    );
    const tool = simulationTools.find((entry) => entry.name === 'simulate_inventory_plan');
    const result = await tool.handler({ commerce: live, params: { plan } });
    assert.equal(result.success, true, JSON.stringify(result));
    assert.equal(await digest(), before);
  } finally {
    if (originalDatabase === undefined) delete process.env.STATESET_DB_PATH;
    else process.env.STATESET_DB_PATH = originalDatabase;
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('model inputs reject database paths, executable content, floats, duplicate identities and unbounded work', async () => {
  assert.equal(
    commerceScenarioSchema.safeParse({ ...scenario, database: '/operator.db' }).success,
    false,
  );
  assert.equal(
    commerceScenarioSchema.safeParse({
      ...scenario,
      inventory: [{ sku: 'SIM', unitPrice: 1.2, quantity: 1 }],
    }).success,
    false,
  );
  assert.equal(commerceScenarioSchema.safeParse({ ...scenario, currency: 'USDC' }).success, false);
  assert.equal(
    commerceScenarioSchema.safeParse({ ...scenario, steps: [{ type: 'eval', code: 'anything' }] })
      .success,
    false,
  );
  assert.equal(
    commerceScenarioSchema.safeParse({ ...scenario, steps: [...scenario.steps, scenario.steps[0]] })
      .success,
    false,
  );
  assert.equal(inventoryPlanSchema.safeParse({ ...plan, days: 61 }).success, false);
  assert.equal(
    inventoryPlanSchema.safeParse({
      ...plan,
      products: [{ ...plan.products[0], demand: { min: 4, max: 1 } }],
    }).success,
    false,
  );
  assert.equal(
    inventoryPlanSchema.safeParse({ ...plan, products: [{ ...plan.products[0], targetStock: 1 }] })
      .success,
    false,
  );
  assert.equal(
    inventoryPlanSchema.safeParse({ ...plan, products: [{ ...plan.products[0], unitCost: 1.25 }] })
      .success,
    false,
  );
  await assert.rejects(compareInventorySimulations([plan, plan]));
});

test('core MCP exposes all native simulation tools as executable reads without --apply', async () => {
  const { createStatesetMcpServer } = await import('../../src/mcp-server.js');
  const { toToolResultContract } = await import('../../src/mcp/tool-result-contract.js');
  const commerce = createCommerce(':memory:');
  const server = createStatesetMcpServer({
    commerce,
    dbPath: ':memory:',
    allowApply: false,
    toolProfile: 'core',
    structuredToolResults: true,
  });
  try {
    await server.servicesReady;
    for (const tool of simulationTools) {
      assert.ok(server.getAdaptedTools().some((entry) => entry.name === tool.name));
    }
    const discovery = toToolResultContract(
      await server.executeTool('get_simulation_kernel_contract', {}),
    );
    assert.equal(discovery.ok, true, JSON.stringify(discovery));
    assert.equal(discovery.preview, false);
    assert.ok(discovery.result.actions.includes('purchase_inventory'));
    assert.equal(discovery.result.liveStoreRead, false);
    const rejected = toToolResultContract(
      await server.executeTool('simulate_inventory_plan', {
        plan: { ...plan, products: Array(20000).fill(plan.products[0]) },
      }),
    );
    assert.equal(rejected.ok, false, JSON.stringify(rejected));
    assert.equal(rejected.preview, false);
    assert.equal(rejected.error.code, 'INVALID_INPUT');
    assert.equal(rejected.error.retryable, false);
    assert.match(rejected.error.hint, /Simulation input/);
    const output = await server.executeTool('simulate_inventory_plan', {
      plan: { ...plan, cashReserve: '20.00' },
    });
    const outcome = toToolResultContract(output);
    assert.equal(outcome.ok, true, JSON.stringify(outcome));
    assert.equal(outcome.preview, false);
    assert.equal(outcome.result.success, true);
    assert.equal(outcome.result.liveStoreModified, false);
    assert.equal(outcome.result.results[0].purchases.length, 0);
    assert.ok(outcome.result.results[0].summary.reserveBlockedPurchases > 0);
    assert.equal(outcome.result.results[0].assumptions.cashReserve, '20.00');
    assert.equal(outcome.result.results[0].reconciliation.cashFlowBalanced, true);
    const supplyOutput = await server.executeTool('simulate_inventory_plan', {
      plan: {
        ...plan,
        name: 'mcp-split-receipts',
        days: 2,
        products: [
          {
            ...plan.products[0],
            initialStock: 0,
            leadTimeDays: { min: 1, max: 1 },
            delivery: { firstReceiptBps: 0, remainderDelayDays: 1 },
            purchasing: { minimumOrderQuantity: 5, orderMultiple: 4 },
          },
        ],
      },
    });
    const supplyOutcome = toToolResultContract(supplyOutput);
    assert.equal(supplyOutcome.ok, true, JSON.stringify(supplyOutcome));
    assert.equal(supplyOutcome.preview, false);
    assert.equal(supplyOutcome.result.results[0].summary.missedDeliveries, 1);
    assert.equal(supplyOutcome.result.results[0].reconciliation.purchaseOrdersBalanced, true);
    assert.equal(
      supplyOutcome.result.results[0].reconciliation.purchasingConstraintsBalanced,
      true,
    );
    assert.equal(supplyOutcome.result.results[0].purchases[0].quantityOrdered, 8);
    const { seed: _seed, ...policy } = plan;
    const experimentOutput = toToolResultContract(
      await server.executeTool('simulate_inventory_experiment', {
        experiment: {
          name: 'mcp-goal-miss',
          seeds: [7, 42],
          plans: [
            {
              ...policy,
              days: 1,
              products: [{ ...policy.products[0], demand: { dailyUnits: [2] } }],
            },
          ],
          goals: {
            minFillRateBps: 10000,
            minEndingCash: '1000',
            productFillRates: [{ sku: 'PLAN', minFillRateBps: 10000 }],
          },
        },
      }),
    );
    assert.equal(experimentOutput.ok, true, JSON.stringify(experimentOutput));
    assert.equal(experimentOutput.preview, false);
    assert.equal(experimentOutput.result.success, true);
    assert.equal(experimentOutput.result.experiment.policies[0].evaluation.missedRuns, 2);
    assert.equal(experimentOutput.result.experiment.policies[0].metrics.demanded.min, 2);
    assert.equal(
      experimentOutput.result.experiment.policies[0].evaluation.byGoal.productFillRates.products[0]
        .metRuns,
      2,
    );
    assert.equal(
      experimentOutput.result.experiment.policies[0].pathDiversity.duplicateJointPathRuns,
      1,
    );
    assert.equal(await commerce.orders.count(), 0);
    assert.equal(await commerce.purchaseOrders.count(), 0);
  } finally {
    await server.close();
    await commerce.close();
  }
});

test('core MCP comparison diagnostics preserve per-alternative conditions', async () => {
  const { createStatesetMcpServer } = await import('../../src/mcp-server.js');
  const { toToolResultContract } = await import('../../src/mcp/tool-result-contract.js');
  const commerce = createCommerce(':memory:');
  const server = createStatesetMcpServer({
    commerce,
    dbPath: ':memory:',
    allowApply: false,
    toolProfile: 'core',
    structuredToolResults: true,
  });
  try {
    await server.servicesReady;
    const comparison = toToolResultContract(
      await server.executeTool('compare_inventory_plans', {
        plans: [
          { ...plan, name: 'baseline-controls', days: 1 },
          {
            ...plan,
            name: 'more-opening-cash',
            days: 1,
            startingCash: '21.00',
            products: [
              {
                ...plan.products[0],
                leadTimeDays: { dailyValues: [1, 1] },
                delivery: {
                  firstReceiptBps: { dailyValues: [10000, 10000] },
                  remainderDelayDays: 1,
                },
              },
            ],
          },
        ],
      }),
    );
    assert.equal(comparison.ok, true, JSON.stringify(comparison));
    assert.equal(comparison.preview, false);
    assert.equal(comparison.result.success, true);
    assert.equal(comparison.result.liveStoreModified, false);
    assert.equal(comparison.result.comparison.sameOpeningCash, false);
    assert.equal(comparison.result.comparison.sameUnitEconomics, true);
    assert.equal(comparison.result.comparison.sameSupplyPath, true);
    assert.equal(
      comparison.result.results[1].supplyProjection.products[0].leadTimeMethod,
      'explicit-daily-values',
    );
    assert.equal(comparison.result.results[1].purchases[0].quantityReceived, 4);
    assert.equal(comparison.result.results[1].reconciliation.inventoryFlowBalanced, true);
    assert.equal(comparison.result.results[1].inventoryFlow.daily.length, 2);
    assert.equal(comparison.result.results[1].inventoryFlow.daily[0].products[0].orderedUnits, 4);
    assert.equal(comparison.result.results[1].inventoryFlow.daily[1].products[0].receivedUnits, 4);
    assert.deepEqual(
      comparison.result.comparison.rows[0].comparisonToBaseline.changedConditions,
      [],
    );
    assert.deepEqual(comparison.result.comparison.rows[1].comparisonToBaseline.changedConditions, [
      'openingCash',
    ]);
    const refused = toToolResultContract(
      await server.executeTool('simulate_inventory_plan', {
        plan: {
          ...plan,
          days: 2,
          products: [{ ...plan.products[0], leadTimeDays: { dailyValues: [1, 1] } }],
        },
      }),
    );
    assert.equal(refused.ok, false);
    assert.equal(refused.error.code, 'INVALID_INPUT');
    assert.equal(refused.error.retryable, false);
    assert.match(refused.error.hint, /days\+1/);
    const unknownTarget = toToolResultContract(
      await server.executeTool('simulate_inventory_experiment', {
        experiment: {
          name: 'unknown-target',
          seeds: [7, 42],
          plans: [{ name: 'one', days: 1, startingCash: '20', products: plan.products }],
          goals: { productFillRates: [{ sku: 'UNKNOWN', minFillRateBps: 10000 }] },
        },
      }),
    );
    assert.equal(unknownTarget.ok, false);
    assert.equal(unknownTarget.error.code, 'INVALID_INPUT');
    assert.equal(unknownTarget.error.retryable, false);
    assert.match(unknownTarget.error.hint, /every policy/);
    const reviewInput = JSON.parse(
      await fs.readFile(
        new URL('../../examples/inventory-review-interval-experiment.json', import.meta.url),
        'utf8',
      ),
    );
    const reviews = toToolResultContract(
      await server.executeTool('simulate_inventory_experiment', { experiment: reviewInput }),
    );
    assert.equal(reviews.ok, true, JSON.stringify(reviews));
    assert.equal(reviews.preview, false);
    assert.equal(reviews.result.experiment.policies[1].metrics.deferredPurchaseReviews.p50, 3);
    assert.equal(reviews.result.experiment.policies[1].reviewDeferredRuns, 2);
    assert.equal(reviews.result.liveStoreModified, false);
    assert.equal(await commerce.orders.count(), 0);
    assert.equal(await commerce.purchaseOrders.count(), 0);
  } finally {
    await server.close();
    await commerce.close();
  }
});

test('native simulation limits simultaneous workers and releases capacity after completion', async () => {
  await assert.rejects(
    runCommerceSimulation({ ...scenario, steps: Array(20000).fill({ type: 'inspect_state' }) }),
    /Simulation input/,
  );
  const first = runCommerceSimulation(scenario);
  const second = runCommerceSimulation(scenario);
  const refused = await runCommerceSimulation(scenario);
  assert.equal(refused.success, false);
  assert.equal(refused.code, 'SIMULATION_BUSY');
  assert.equal(refused.retryable, true);
  const contract = normalizeToolResult(refused);
  assert.equal(contract.error.code, 'SIMULATION_BUSY');
  assert.equal(contract.error.retryable, true);
  assert.equal(refused.liveStoreModified, false);
  const completed = await Promise.all([first, second]);
  assert.ok(completed.every((entry) => entry.success));
  assert.equal((await runCommerceSimulation(scenario)).success, true);
});
