import { subtractDecimals } from '../utils/exact-decimal.js';
import { inventoryComparisonContext } from '../simulation/comparison-context.js';
import { SIMULATION_LIMITS } from '../simulation/limits.js';

// undefined means unavailable; null is the engine's recorded absence of unmet
// demand. Empty ending stock alone cannot establish a stockout.
export function recordedFirstStockout(report) {
  if (
    !Number.isSafeInteger(report?.days) ||
    report.days < 1 ||
    report.days > SIMULATION_LIMITS.maxPlanningDays
  )
    return undefined;
  const first = report.inventoryFlow?.firstStockout;
  if (first === null) return null;
  if (
    !first ||
    typeof first !== 'object' ||
    Array.isArray(first) ||
    typeof first.sku !== 'string' ||
    !first.sku ||
    !Number.isSafeInteger(first.day) ||
    first.day < 1 ||
    first.day > report.days ||
    ![first.demanded, first.sold, first.lostSales].every(
      (value) => Number.isSafeInteger(value) && value >= 0,
    ) ||
    first.lostSales < 1 ||
    first.demanded - first.sold !== first.lostSales
  )
    return undefined;
  return first;
}

// Compare recorded SKU policy facts independently of priority, cash and paths.
// Unknown future rules stay unavailable instead of being silently ignored.
export function purchasingRulesMatch(reports) {
  if (!Array.isArray(reports) || reports.length !== 2) return null;
  try {
    const policies = reports.map((report) => {
      const products = report?.assumptions?.products;
      if (!Array.isArray(products) || !products.length) throw new Error('Missing SKU policies');
      const skus = new Set();
      const integer = (value, minimum, maximum = SIMULATION_LIMITS.maxPurchaseQuantity) => {
        if (!Number.isSafeInteger(value) || value < minimum || value > maximum)
          throw new Error('Invalid policy quantity');
        return value;
      };
      return products
        .map((product) => {
          if (typeof product?.sku !== 'string' || !product.sku || skus.has(product.sku))
            throw new Error('Invalid SKU policies');
          skus.add(product.sku);
          const terms = product.purchasing === undefined ? {} : product.purchasing;
          if (
            !terms ||
            typeof terms !== 'object' ||
            Array.isArray(terms) ||
            Object.keys(terms).some(
              (key) =>
                ![
                  'minimumOrderQuantity',
                  'orderMultiple',
                  'reviewIntervalDays',
                  'maximumInventoryPosition',
                ].includes(key),
            )
          )
            throw new Error('Unknown purchasing rules');
          const reorder = integer(product.reorderPoint, 0);
          const target = integer(product.targetStock, 1);
          if (target <= reorder) throw new Error('Invalid stock targets');
          return [
            product.sku,
            reorder,
            target,
            integer(terms.minimumOrderQuantity === undefined ? 1 : terms.minimumOrderQuantity, 1),
            integer(terms.orderMultiple === undefined ? 1 : terms.orderMultiple, 1),
            integer(
              terms.reviewIntervalDays === undefined ? 1 : terms.reviewIntervalDays,
              1,
              SIMULATION_LIMITS.maxPlanningDays,
            ),
            terms.maximumInventoryPosition === undefined
              ? null
              : integer(terms.maximumInventoryPosition, 1),
          ];
        })
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    });
    return JSON.stringify(policies[0]) === JSON.stringify(policies[1]);
  } catch {
    return null;
  }
}

export function comparisonEvidence(reports) {
  if (!Array.isArray(reports) || reports.length !== 2) return null;
  const [baseline, candidate] = reports;
  if (!baseline?.currency || baseline.currency !== candidate?.currency) return null;
  if (
    reports.some(
      (report) =>
        !Number.isSafeInteger(report?.days) ||
        report.days < 1 ||
        !Array.isArray(report.assumptions?.products) ||
        !report.assumptions.products.length ||
        !Array.isArray(report.demandForecast?.products) ||
        !report.demandForecast.products.length ||
        !Array.isArray(report.supplyProjection?.products) ||
        !report.supplyProjection.products.length,
    )
  )
    return null;
  const conditions = (report) => ({
    days: report.days,
    startingCash: report.summary?.startingCash,
    cashReserve: report.assumptions.cashReserve,
    products: report.assumptions.products,
  });
  try {
    return inventoryComparisonContext(
      conditions(baseline),
      conditions(candidate),
      [baseline],
      [candidate],
    );
  } catch {
    // Old saved runs may lack evidence required by the current engine checks.
    return null;
  }
}

// Refusals are timeline attempts, including repeated days and day zero.
// Preserve exact recorded money; this is presentation, not purchase sizing.
export function cashBudgetRefusals(report) {
  return purchaseAttempts(report, new Set(['purchase_budget_blocked']));
}

// Group only recorded decisions, separately by event type and SKU. A review
// deferral is not a refusal or a promise that a later order will be placed.
export function purchasingExceptions(report) {
  return purchaseAttempts(
    report,
    new Set([
      'purchase_constraint_blocked',
      'purchase_reserve_blocked',
      'purchase_review_deferred',
    ]),
  );
}

function purchaseAttempts(report, types) {
  if (!Array.isArray(report?.timeline)) return null;
  const groups = [];
  const byType = new Map();
  for (const event of report.timeline) {
    if (!types.has(event?.type)) continue;
    const sku = typeof event.sku === 'string' ? event.sku : '(unknown SKU)';
    if (!byType.has(event.type)) byType.set(event.type, new Map());
    const bySku = byType.get(event.type);
    let entry = bySku.get(sku);
    if (!entry) {
      entry = { sku, attempts: 0, first: event };
      bySku.set(sku, entry);
      groups.push(entry);
    }
    entry.attempts += 1;
  }
  return groups;
}

export function timelineEntries(report, query = '') {
  if (!Array.isArray(report?.timeline)) return null;
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const entries = report.timeline.map((event, index) => ({ event, index }));
  return {
    total: entries.length,
    entries: entries.filter(({ event }) => {
      const type = typeof event?.type === 'string' ? event.type : '';
      const sku = typeof event?.sku === 'string' ? event.sku : '';
      const label = `${type} ${type.replace(/_/g, ' ')} ${sku}`.toLocaleLowerCase();
      return terms.every((term) => label.includes(term));
    }),
  };
}

export function fillRate(summary) {
  if (
    !Number.isSafeInteger(summary?.demanded) ||
    summary.demanded <= 0 ||
    !Number.isSafeInteger(summary?.sold) ||
    summary.sold < 0 ||
    summary.sold > summary.demanded
  )
    return 'n/a';
  const bps = Math.floor((summary.sold * 10000) / summary.demanded);
  return `${Math.floor(bps / 100)}.${String(bps % 100).padStart(2, '0')}%`;
}

export function cashDelta(candidate, baseline, field) {
  if (
    typeof baseline?.currency !== 'string' ||
    !baseline.currency.trim() ||
    candidate?.currency !== baseline?.currency ||
    typeof candidate?.summary?.[field] !== 'string' ||
    typeof baseline?.summary?.[field] !== 'string'
  )
    return 'n/a';
  try {
    const result = subtractDecimals(candidate.summary[field], baseline.summary[field]);
    return result.startsWith('-') ? result : `+${result}`;
  } catch {
    return 'n/a';
  }
}

// This is a change in cash, not income: purchases can turn cash into stock.
// Use executed report evidence only; an edited draft cannot fill missing data.
export function cashMovement(report) {
  if (
    typeof report?.currency !== 'string' ||
    !report.currency.trim() ||
    typeof report.summary?.startingCash !== 'string' ||
    typeof report.summary?.endingCash !== 'string'
  )
    return null;
  try {
    return subtractDecimals(report.summary.endingCash, report.summary.startingCash);
  } catch {
    return null;
  }
}

export function cashMovementDelta(candidate, baseline) {
  if (
    candidate?.currency !== baseline?.currency ||
    !Number.isSafeInteger(baseline?.days) ||
    baseline.days < 1 ||
    candidate?.days !== baseline.days
  )
    return null;
  const a = cashMovement(candidate),
    b = cashMovement(baseline);
  if (a === null || b === null) return null;
  const result = subtractDecimals(a, b);
  return result.startsWith('-') ? result : `+${result}`;
}

export function soldTraces(reports, maxColumns = 24) {
  const days = Math.max(1, ...reports.map((r) => r.days || 1));
  const bucket = Math.max(1, Math.ceil(days / Math.max(1, maxColumns)));
  const values = reports.map((report) => {
    const bins = Array(Math.ceil(days / bucket)).fill(0);
    for (const event of report.timeline || []) {
      if (
        event.type === 'demand' &&
        Number.isInteger(event.day) &&
        event.day > 0 &&
        event.day <= days &&
        Number.isSafeInteger(event.sold) &&
        event.sold >= 0
      )
        bins[Math.floor((event.day - 1) / bucket)] += event.sold;
    }
    return bins;
  });
  const peak = Math.max(0, ...values.flat());
  const levels = '▁▂▃▄▅▆▇█';
  return {
    bucket,
    peak,
    lines: values.map((bins) =>
      bins
        .map((value) =>
          value === 0 ? '·' : levels[Math.min(7, Math.ceil((value / peak) * 8) - 1)],
        )
        .join(''),
    ),
  };
}
