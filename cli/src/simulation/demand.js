import { createSeededSampler } from './random.js';

export function demandMethod(demand) {
  return demand.dailyUnits
    ? 'explicit-daily-units'
    : demand.history
      ? 'empirical-history-resampling'
      : 'uniform-integer';
}

// Accepts a validated plan. Each SKU has its own stream so changing a policy,
// adding another product, or reordering products cannot change its demand.
export function projectDemand(plan) {
  return {
    generator: 'mulberry32-sku-v1',
    seed: plan.seed,
    days: plan.days,
    products: plan.products.map(({ sku, demand }) => {
      const sample = createSeededSampler(`${plan.seed}:${sku}`);
      const dailyUnits = demand.dailyUnits
        ? [...demand.dailyUnits]
        : Array.from({ length: plan.days }, () =>
            demand.history
              ? demand.history[sample(demand.history.length)]
              : demand.min + sample(demand.max - demand.min + 1),
          );
      const projectedUnits = dailyUnits.reduce((sum, units) => sum + units, 0);
      const expectedDailyUnits = demand.dailyUnits
        ? projectedUnits / plan.days
        : demand.history
          ? demand.history.reduce((sum, units) => sum + units, 0) / demand.history.length
          : (demand.min + demand.max) / 2;
      return {
        sku,
        method: demandMethod(demand),
        expectedDailyUnits,
        expectedHorizonUnits: demand.dailyUnits ? projectedUnits : expectedDailyUnits * plan.days,
        projectedUnits,
        dailyUnits,
      };
    }),
  };
}
