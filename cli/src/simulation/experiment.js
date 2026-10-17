import { addDecimals, subtractDecimals, compareDecimals } from '../utils/exact-decimal.js';
import { inventoryExperimentSchema } from './schema.js';
import { MONEY_METRICS, UNIT_METRICS } from './comparison.js';
import { hashSupplyProjection } from './supply.js';
import { serviceLevel, evaluateInventoryGoals, summarizeGoalEvaluations } from './evaluation.js';
import {
  INVENTORY_CONDITION_FLAGS,
  hashDemandProjection,
  inventoryComparisonContext,
} from './comparison-context.js';

function distribution(values, money) {
  const sorted = [...values].sort(money ? compareDecimals : (a, b) => a - b);
  const percentile = (percent) =>
    sorted[Math.max(0, Math.ceil((sorted.length * percent) / 100) - 1)];
  const sum = money ? addDecimals(values) : values.reduce((total, value) => total + value, 0);
  return {
    min: sorted[0],
    p10: percentile(10),
    p50: percentile(50),
    p90: percentile(90),
    max: sorted[sorted.length - 1],
    // Preserve exact money: a rational mean does not round or introduce floats.
    mean: money ? { numerator: sum, denominator: values.length } : sum / values.length,
  };
}

function metrics(reports, baseline) {
  return Object.fromEntries(
    [...UNIT_METRICS, ...MONEY_METRICS].map((key) => {
      const money = MONEY_METRICS.includes(key);
      const values = reports.map((report, index) =>
        baseline
          ? money
            ? subtractDecimals(report.summary[key], baseline[index].summary[key])
            : report.summary[key] - baseline[index].summary[key]
          : report.summary[key],
      );
      return [key, distribution(values, money)];
    }),
  );
}

function demandHash(report) {
  return hashDemandProjection(report.demandForecast);
}

function pathDiversity(reports) {
  const paths = reports.map((report) => [
    demandHash(report),
    hashSupplyProjection(report.supplyProjection),
  ]);
  const demandPaths = new Set(paths.map(([demand]) => demand)).size;
  const supplyPaths = new Set(paths.map(([, supply]) => supply)).size;
  const jointPaths = new Set(paths.map((path) => JSON.stringify(path))).size;
  return {
    demandPaths,
    supplyPaths,
    jointPaths,
    duplicateJointPathRuns: reports.length - jointPaths,
  };
}

// Pure aggregation over completed, independently reconciled native episodes.
export function summarizeInventoryExperiment(input, reports) {
  const experiment = inventoryExperimentSchema.parse(input);
  const key = (name, seed) => JSON.stringify([name, seed]);
  const indexed = new Map(reports.map((report) => [key(report.name, report.seed), report]));
  const expectedRuns = experiment.plans.length * experiment.seeds.length;
  if (indexed.size !== expectedRuns || reports.length !== expectedRuns)
    throw new Error('Experiment requires exactly one native report per plan and seed');
  const cases = experiment.plans.map((plan) =>
    experiment.seeds.map((seed) => {
      const report = indexed.get(key(plan.name, seed));
      if (
        !report ||
        report.currency !== plan.currency ||
        report.days !== plan.days ||
        !report.supplyProjection ||
        ![
          'stockBalanced',
          'cashBalanced',
          'cashFlowBalanced',
          'inventoryFlowBalanced',
          'inventoryValueBalanced',
          'purchaseOrdersBalanced',
          'purchasingConstraintsBalanced',
        ].every((field) => report.reconciliation?.[field] === true)
      )
        throw new Error('Experiment native report identity or reconciliation failed');
      return report;
    }),
  );
  const baseline = cases[0];
  const contexts = experiment.plans.map((plan, index) =>
    inventoryComparisonContext(experiment.plans[0], plan, baseline, cases[index]),
  );
  return {
    name: experiment.name,
    currency: experiment.plans[0].currency,
    seeds: experiment.seeds,
    nativeRuns: expectedRuns,
    baseline: experiment.plans[0].name,
    interpretation: 'empirical-outcomes-over-supplied-seeds',
    percentileMethod:
      'nearest-rank; p10/p50/p90 are observed sample outcomes, not confidence bounds',
    pathDiversityDefinition:
      'Distinct hashes of full projected demand and supplier paths within each policy. Repeated joint paths retain their supplied sample weight. Counts are not distinct financial outcomes, independent observations or an effective statistical sample size.',
    moneyMeanEncoding:
      'exact rational: numerator is the sum of decimal amounts; denominator is the number of samples',
    supplyDisruptionDefinition:
      'at-least-one-partial-or-missed-first-delivery-observed-within-horizon',
    serviceLevelDefinition:
      'Unit fill rate is total sold / total demanded, pooled across runs rather than averaged percentages; zero demand returns null.',
    goalEvaluationSemantics:
      'Inclusive thresholds checked independently on every seed. All supplied goals must be met in the same run. No-demand fill-rate goals are not-evaluable; goal misses do not indicate engine failure or calibrated probabilities.',
    ...Object.fromEntries(
      INVENTORY_CONDITION_FLAGS.map((flag) => [flag, contexts.every((context) => context[flag])]),
    ),
    sameDemandPaths: contexts.every((context) => context.sameDemandPaths),
    sameSupplyPaths: contexts.every((context) => context.sameSupplyPaths),
    policies: experiment.plans.map((plan, index) => {
      const runs = cases[index];
      const evaluations = runs.map((run) =>
        evaluateInventoryGoals(run.summary, experiment.goals, run.inventory),
      );
      return {
        plan,
        comparisonToBaseline: contexts[index],
        samples: runs.length,
        pathDiversity: pathDiversity(runs),
        serviceLevel: {
          ...serviceLevel(runs.map((run) => run.summary)),
          zeroDemandRuns: runs.filter((run) => run.summary.demanded === 0).length,
        },
        evaluation: summarizeGoalEvaluations(experiment.goals, evaluations),
        stockoutRuns: runs.filter((run) => run.summary.lostSales > 0).length,
        budgetBlockedRuns: runs.filter((run) =>
          run.timeline.some((event) => event.type === 'purchase_budget_blocked'),
        ).length,
        constraintBlockedRuns: runs.filter((run) => run.summary.constraintBlockedPurchases > 0)
          .length,
        reserveBlockedRuns: runs.filter((run) => run.summary.reserveBlockedPurchases > 0).length,
        reviewDeferredRuns: runs.filter((run) => run.summary.deferredPurchaseReviews > 0).length,
        disruptedSupplyRuns: runs.filter(
          (run) => run.summary.partialReceipts > 0 || run.summary.missedDeliveries > 0,
        ).length,
        metrics: metrics(runs),
        pairedDeltaFromBaseline: {
          metrics: metrics(runs, baseline),
          signs: Object.fromEntries(
            [...UNIT_METRICS, ...MONEY_METRICS].map((metric) => {
              const signs = runs.map((run, seedIndex) =>
                MONEY_METRICS.includes(metric)
                  ? compareDecimals(run.summary[metric], baseline[seedIndex].summary[metric])
                  : Math.sign(run.summary[metric] - baseline[seedIndex].summary[metric]),
              );
              return [
                metric,
                {
                  positive: signs.filter((sign) => sign > 0).length,
                  zero: signs.filter((sign) => sign === 0).length,
                  negative: signs.filter((sign) => sign < 0).length,
                },
              ];
            }),
          ),
        },
        runs: runs.map((report, runIndex) => ({
          seed: report.seed,
          demandHash: demandHash(report),
          supplyHash: hashSupplyProjection(report.supplyProjection),
          summary: report.summary,
          cashFlow: report.cashFlow,
          inventoryFlow: report.inventoryFlow,
          serviceLevel: serviceLevel([report.summary]),
          evaluation: evaluations[runIndex],
          inventory: report.inventory,
          reconciliation: report.reconciliation,
        })),
      };
    }),
  };
}
