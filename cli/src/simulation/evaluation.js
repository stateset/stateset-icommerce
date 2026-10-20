import { compareDecimals } from '../utils/exact-decimal.js';

// Keep the fraction exact; a no-demand episode supplies no service-level evidence.
export function serviceLevel(summaries) {
  const total = { demanded: 0, sold: 0, lostSales: 0 };
  for (const summary of summaries) {
    if (
      !Object.keys(total).every((key) => Number.isSafeInteger(summary[key]) && summary[key] >= 0) ||
      summary.sold + summary.lostSales !== summary.demanded
    )
      throw new Error('Service level requires balanced nonnegative integer demand and sales');
    for (const key of Object.keys(total)) {
      total[key] += summary[key];
      if (!Number.isSafeInteger(total[key]))
        throw new Error('Service level total exceeds safe integer range');
    }
  }
  return {
    ...total,
    fillRate: total.demanded === 0 ? null : { numerator: total.sold, denominator: total.demanded },
  };
}

function jointStatus(statuses) {
  return statuses.includes('missed')
    ? 'missed'
    : statuses.includes('not-evaluable')
      ? 'not-evaluable'
      : 'met';
}

function fillRateCheck(summary, threshold) {
  const actual = serviceLevel([summary]).fillRate;
  return {
    threshold,
    actual,
    status:
      actual === null
        ? 'not-evaluable'
        : BigInt(actual.numerator) * 10000n >= BigInt(threshold) * BigInt(actual.denominator)
          ? 'met'
          : 'missed',
  };
}

export function evaluateInventoryGoals(summary, goals, inventory = []) {
  if (!goals) return null;
  const checks = {};
  for (const [goal, threshold] of Object.entries(goals)) {
    if (threshold === undefined) continue;
    if (goal === 'minFillRateBps') {
      checks[goal] = fillRateCheck(summary, threshold);
    } else if (goal === 'productFillRates') {
      const products = threshold.map(({ sku, minFillRateBps }) => {
        const matches = inventory.filter((item) => item.sku === sku);
        if (matches.length !== 1)
          throw new Error(
            'Product fill-rate goals require exactly one native inventory row per SKU',
          );
        return { sku, ...fillRateCheck(matches[0], minFillRateBps) };
      });
      checks[goal] = { status: jointStatus(products.map((product) => product.status)), products };
    } else {
      const field = {
        minEndingCash: 'endingCash',
        minCashBalance: 'minimumCash',
        minGrossMargin: 'grossMargin',
        maxPurchasingSpend: 'purchasingSpend',
      }[goal];
      if (!field) throw new Error('Unknown inventory goal');
      const actual = summary[field];
      const comparison = compareDecimals(actual, threshold);
      checks[goal] = {
        threshold,
        actual,
        status: (goal === 'maxPurchasingSpend' ? comparison <= 0 : comparison >= 0)
          ? 'met'
          : 'missed',
      };
    }
  }
  const statuses = Object.values(checks).map((check) => check.status);
  return {
    status: jointStatus(statuses),
    checks,
  };
}

export function summarizeGoalEvaluations(goals, evaluations) {
  if (!goals) return null;
  const counts = (statuses) => ({
    metRuns: statuses.filter((status) => status === 'met').length,
    missedRuns: statuses.filter((status) => status === 'missed').length,
    notEvaluableRuns: statuses.filter((status) => status === 'not-evaluable').length,
  });
  const statuses = evaluations.map((evaluation) => evaluation.status);
  return {
    goals,
    ...counts(statuses),
    allRunsMet: statuses.every((status) => status === 'met'),
    byGoal: Object.fromEntries(
      Object.keys(goals)
        .filter((goal) => goals[goal] !== undefined)
        .map((goal) => [
          goal,
          {
            ...counts(evaluations.map((evaluation) => evaluation.checks[goal].status)),
            ...(goal === 'productFillRates'
              ? {
                  products: goals.productFillRates.map(({ sku, minFillRateBps }) => {
                    const checks = evaluations.map((evaluation) =>
                      evaluation.checks.productFillRates.products.find(
                        (product) => product.sku === sku,
                      ),
                    );
                    return {
                      sku,
                      threshold: minFillRateBps,
                      ...counts(checks.map((check) => check.status)),
                      serviceLevel: serviceLevel(
                        checks.map(({ actual }) => ({
                          demanded: actual?.denominator ?? 0,
                          sold: actual?.numerator ?? 0,
                          lostSales: actual ? actual.denominator - actual.numerator : 0,
                        })),
                      ),
                    };
                  }),
                }
              : {}),
          },
        ]),
    ),
  };
}
