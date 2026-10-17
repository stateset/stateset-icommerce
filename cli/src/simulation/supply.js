import { createSeededSampler } from './random.js';
import { createHash } from 'node:crypto';

export function hashSupplyProjection(projection) {
  const entries = projection.products.map((item) => [
    item.sku,
    item.dailyLeadTimeDays,
    item.dailyFirstReceiptBps,
    item.remainderDelayDays,
  ]);
  entries.sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));
  return createHash('sha256').update(JSON.stringify(entries)).digest('hex');
}

function samples(input, seed, sku, stream, length) {
  if (typeof input === 'number') return Array(length).fill(input);
  if (input.dailyValues) return [...input.dailyValues];
  const sample = createSeededSampler(`${seed}:${sku}:${stream}:v1`);
  return Array.from({ length }, () => input.min + sample(input.max - input.min + 1));
}

function method(input) {
  return typeof input === 'number'
    ? 'fixed'
    : input.dailyValues
      ? 'explicit-daily-values'
      : 'uniform-integer';
}

// Supplier conditions are indexed by SKU and order-placement day (including
// day zero), independently of demand, quantities, policy and product ordering.
export function projectSupply(plan) {
  return {
    generator: 'mulberry32-supply-v1',
    seed: plan.seed,
    days: plan.days,
    indexing: 'sku-and-order-placement-day; includes-day-zero',
    products: plan.products.map((product) => ({
      sku: product.sku,
      leadTimeMethod: method(product.leadTimeDays),
      firstReceiptMethod: method(product.delivery?.firstReceiptBps ?? 10000),
      dailyLeadTimeDays: samples(
        product.leadTimeDays,
        plan.seed,
        product.sku,
        'lead-time',
        plan.days + 1,
      ),
      dailyFirstReceiptBps: samples(
        product.delivery?.firstReceiptBps ?? 10000,
        plan.seed,
        product.sku,
        'first-receipt',
        plan.days + 1,
      ),
      remainderDelayDays: product.delivery?.remainderDelayDays ?? 1,
    })),
  };
}
