import { inventoryPlanSchema } from '../simulation/schema.js';

export const PRODUCT_FIELDS = [
  { key: 'initialStock', label: 'Opening stock', hint: 'Whole units, 0–500', min: 0, max: 500 },
  {
    key: 'unitPrice',
    label: 'Sale price',
    hint: 'Exact USD amount, greater than zero',
    money: true,
  },
  {
    key: 'unitCost',
    label: 'Purchase unit cost',
    hint: 'Required assumption; never inferred from price',
    money: true,
  },
  { key: 'demandMin', label: 'Daily demand minimum', hint: 'Whole units, 0–100', min: 0, max: 100 },
  {
    key: 'demandMax',
    label: 'Daily demand maximum',
    hint: 'At least minimum; spike multiplies both by 3',
    min: 0,
    max: 100,
  },
  { key: 'reorderPoint', label: 'Reorder point', hint: 'Whole units, 0–250', min: 0, max: 250 },
  {
    key: 'targetStock',
    label: 'Target stock',
    hint: 'Above reorder point; at most 500',
    min: 1,
    max: 500,
  },
  {
    key: 'leadTimeDays',
    label: 'Supplier lead time',
    hint: '1–14 days; delay adds 3',
    min: 1,
    max: 14,
  },
  { key: 'minimumOrderQuantity', label: 'Supplier minimum', hint: '1–500 units', min: 1, max: 500 },
  { key: 'orderMultiple', label: 'Supplier case pack', hint: '1–100 units', min: 1, max: 100 },
];

export function productEditor(row, index = -1) {
  return {
    kind: 'product',
    field: 0,
    index,
    sku: row.sku,
    error: null,
    draft: {
      initialStock: String(row.initialStock ?? row.available ?? ''),
      unitPrice: row.unitPrice ?? row.priceExact ?? '',
      unitCost: row.unitCost ?? '',
      demandMin: String(row.demand?.min ?? 2),
      demandMax: String(row.demand?.max ?? 5),
      reorderPoint: String(row.reorderPoint ?? 8),
      targetStock: String(row.targetStock ?? 24),
      leadTimeDays: String(row.leadTimeDays ?? 1),
      minimumOrderQuantity: String(row.purchasing?.minimumOrderQuantity ?? 1),
      orderMultiple: String(row.purchasing?.orderMultiple ?? 1),
    },
  };
}

export function productFromDraft(sku, draft) {
  const values = {};
  for (const field of PRODUCT_FIELDS) {
    const value = draft[field.key];
    if (field.money) {
      if (
        typeof value !== 'string' ||
        !/^(0|[1-9]\d{0,5})(\.\d{1,2})?$/.test(value) ||
        (field.key === 'unitPrice' && !/[1-9]/.test(value))
      )
        throw new Error(`${field.label}: ${field.hint}`);
      values[field.key] = value;
    } else {
      if (
        !/^\d{1,6}$/.test(String(value)) ||
        Number(value) < field.min ||
        Number(value) > field.max
      )
        throw new Error(`${field.label}: ${field.hint}`);
      values[field.key] = Number(value);
    }
  }
  if (values.demandMax < values.demandMin)
    throw new Error('Demand maximum must be at least minimum');
  const { demandMin, demandMax, minimumOrderQuantity, orderMultiple, ...product } = values;
  const result = {
    sku,
    ...product,
    demand: { min: demandMin, max: demandMax },
    purchasing: { minimumOrderQuantity, orderMultiple },
  };
  inventoryPlanSchema.parse({
    name: 'Validate product',
    days: 1,
    startingCash: '0.00',
    products: [result],
  });
  return result;
}

export function planningProducts(input) {
  if (!Array.isArray(input) || input.length > 3) throw new Error('Choose at most 3 SKUs');
  const products = input.map((p) => {
    if (
      Object.keys(p).some(
        (key) =>
          ![
            'sku',
            'initialStock',
            'unitPrice',
            'unitCost',
            'demand',
            'reorderPoint',
            'targetStock',
            'leadTimeDays',
            'purchasing',
          ].includes(key),
      ) ||
      Object.keys(p.purchasing || {}).some(
        (key) => !['minimumOrderQuantity', 'orderMultiple'].includes(key),
      )
    )
      throw new Error('Unsupported SKU planning terms');
    const normalized = productFromDraft(p.sku, productEditor(p).draft);
    // Persisted/imported inputs must not silently lose unsupported properties.
    const parsed = inventoryPlanSchema.parse({
      name: 'Validate product',
      days: 1,
      startingCash: '0.00',
      products: [p],
    }).products[0];
    if (JSON.stringify(parsed.demand) !== JSON.stringify(normalized.demand))
      throw new Error('Use bounded daily demand minimum/maximum');
    return normalized;
  });
  if (new Set(products.map((p) => p.sku)).size !== products.length)
    throw new Error('Product SKUs must be unique');
  return products;
}
