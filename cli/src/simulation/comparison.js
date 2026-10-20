import { subtractDecimals } from '../utils/exact-decimal.js';
import { INVENTORY_CONDITION_FLAGS, inventoryComparisonContext } from './comparison-context.js';

export const MONEY_METRICS = [
  'revenue',
  'lostRevenue',
  'purchasingSpend',
  'endingCash',
  'minimumCash',
  'grossMargin',
  'endingInventoryValue',
  'inTransitInventoryValue',
];
export const UNIT_METRICS = [
  'demanded',
  'sold',
  'lostSales',
  'orders',
  'purchaseOrders',
  'partialReceipts',
  'missedDeliveries',
  'constraintBlockedPurchases',
  'reserveBlockedPurchases',
  'deferredPurchaseReviews',
  'roundedPurchaseOrders',
  'extraPurchasedUnits',
];

export function compareInventoryReports(reports) {
  const baseline = reports[0];
  const conditions = (report) => ({
    days: report.days,
    startingCash: report.summary.startingCash,
    cashReserve: report.assumptions.cashReserve,
    products: report.assumptions.products,
  });
  const baselineConditions = conditions(baseline);
  const contexts = reports.map((report) =>
    inventoryComparisonContext(baselineConditions, conditions(report), [baseline], [report]),
  );
  return {
    baseline: baseline.name,
    ...Object.fromEntries(
      INVENTORY_CONDITION_FLAGS.map((flag) => [flag, contexts.every((context) => context[flag])]),
    ),
    sameSupplyPath: contexts.every((context) => context.sameSupplyPaths),
    sameDemandPath: contexts.every((context) => context.sameDemandPaths),
    rows: reports.map((report, index) => ({
      name: report.name,
      currency: report.currency,
      days: report.days,
      comparisonToBaseline: contexts[index],
      metrics: Object.fromEntries(
        [...UNIT_METRICS, ...MONEY_METRICS].map((key) => [key, report.summary[key]]),
      ),
      deltaFromBaseline: Object.fromEntries([
        ...UNIT_METRICS.map((key) => [key, report.summary[key] - baseline.summary[key]]),
        ...MONEY_METRICS.map((key) => [
          key,
          subtractDecimals(report.summary[key], baseline.summary[key]),
        ]),
      ]),
    })),
  };
}
