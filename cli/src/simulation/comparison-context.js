import { createHash } from 'node:crypto';
import { compareDecimals, formatDecimal } from '../utils/exact-decimal.js';
import { hashSupplyProjection } from './supply.js';

export const INVENTORY_CONDITION_FLAGS = [
  'sameHorizon',
  'sameOpeningCash',
  'sameCashReserve',
  'sameOpeningInventory',
  'sameUnitEconomics',
  'sameProductPriority',
];

function productFacts(plan, fields) {
  return JSON.stringify(
    plan.products
      .map((product) => [
        product.sku,
        ...fields.map((field) =>
          field === 'initialStock' ? product[field] : formatDecimal(product[field]),
        ),
      ])
      .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0)),
  );
}

export function hashDemandProjection(projection) {
  const entries = projection.products.map((item) => [item.sku, item.dailyUnits]);
  entries.sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));
  return createHash('sha256').update(JSON.stringify(entries)).digest('hex');
}

// Runs are already paired by seed by the experiment aggregator. Path matches
// describe these projected runs; they do not establish equivalent assumptions.
export function inventoryComparisonContext(baseline, alternative, baselineRuns, runs) {
  const paired = runs.length > 0 && runs.length === baselineRuns.length;
  const flags = {
    sameHorizon: alternative.days === baseline.days,
    sameOpeningCash: compareDecimals(alternative.startingCash, baseline.startingCash) === 0,
    sameCashReserve:
      compareDecimals(alternative.cashReserve ?? '0', baseline.cashReserve ?? '0') === 0,
    sameOpeningInventory:
      productFacts(alternative, ['initialStock']) === productFacts(baseline, ['initialStock']),
    sameUnitEconomics:
      productFacts(alternative, ['unitPrice', 'unitCost']) ===
      productFacts(baseline, ['unitPrice', 'unitCost']),
    sameProductPriority:
      JSON.stringify(alternative.products.map((product) => product.sku)) ===
      JSON.stringify(baseline.products.map((product) => product.sku)),
    sameDemandPaths:
      paired &&
      runs.every(
        (run, index) =>
          hashDemandProjection(run.demandForecast) ===
          hashDemandProjection(baselineRuns[index].demandForecast),
      ),
    sameSupplyPaths:
      paired &&
      runs.every(
        (run, index) =>
          Boolean(run.supplyProjection && baselineRuns[index].supplyProjection) &&
          hashSupplyProjection(run.supplyProjection) ===
            hashSupplyProjection(baselineRuns[index].supplyProjection),
      ),
  };
  const labels = {
    sameHorizon: 'horizon',
    sameOpeningCash: 'openingCash',
    sameCashReserve: 'cashReserve',
    sameOpeningInventory: 'openingInventory',
    sameUnitEconomics: 'unitEconomics',
    sameProductPriority: 'productPriority',
    sameDemandPaths: 'demandPath',
    sameSupplyPaths: 'supplyPath',
  };
  return {
    ...flags,
    changedConditions: Object.keys(flags)
      .filter((field) => !flags[field])
      .map((field) => labels[field]),
  };
}
