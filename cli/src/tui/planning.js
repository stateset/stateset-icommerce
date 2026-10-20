import { formatDecimal, multiplyDecimals } from '../utils/exact-decimal.js';

export const DEFAULT_PLANNING = Object.freeze({
  days: 7,
  startingCash: '200.00',
  initialStock: 12,
  reorderPoint: 8,
  targetStock: 24,
  leadTimeDays: 1,
  cashReserve: '0.00',
});

export const PLANNING_FIELDS = [
  { key: 'days', label: 'Planning horizon', hint: '1–30 days', min: 1, max: 30 },
  { key: 'startingCash', label: 'Opening cash (USD)', hint: '0–999999.99; exact decimal' },
  { key: 'initialStock', label: 'Opening stock', hint: '0–500 units', min: 0, max: 500 },
  { key: 'reorderPoint', label: 'Reorder point', hint: '0–250 units', min: 0, max: 250 },
  {
    key: 'targetStock',
    label: 'Target stock',
    hint: '1–500; above reorder point',
    min: 1,
    max: 500,
  },
  { key: 'leadTimeDays', label: 'Supplier lead time', hint: '1–14 days', min: 1, max: 14 },
  {
    key: 'cashReserve',
    label: 'Cash reserve (USD)',
    hint: '0–999999.99; withheld from purchases; may exceed opening cash',
  },
];

export function planningFields(products = []) {
  return products.length
    ? PLANNING_FIELDS.filter(({ key }) => ['days', 'startingCash', 'cashReserve'].includes(key))
    : PLANNING_FIELDS;
}

export function planningControls(input = DEFAULT_PLANNING) {
  if (!input || typeof input !== 'object' || Array.isArray(input))
    throw new Error('Planning assumptions must be an object');
  for (const key of Object.keys(input))
    if (!PLANNING_FIELDS.some((field) => field.key === key))
      throw new Error(`Unknown planning assumption: ${key}`);
  const values = { ...DEFAULT_PLANNING, ...input },
    result = {};
  for (const field of PLANNING_FIELDS) {
    const value = values[field.key];
    if (['startingCash', 'cashReserve'].includes(field.key)) {
      if (typeof value !== 'string' || !/^(0|[1-9]\d{0,5})(\.\d{1,2})?$/.test(value))
        throw new Error(
          `${field.key === 'cashReserve' ? 'Cash reserve' : 'Opening cash'} must be a decimal string from 0 to 999999.99`,
        );
      result[field.key] = formatDecimal(value);
    } else {
      if (
        !/^\d{1,6}$/.test(String(value)) ||
        !Number.isSafeInteger(Number(value)) ||
        Number(value) < field.min ||
        Number(value) > field.max
      )
        throw new Error(`${field.label}: ${field.hint}`);
      result[field.key] = Number(value);
    }
  }
  if (result.targetStock <= result.reorderPoint)
    throw new Error('Target stock must be above the reorder point');
  return result;
}

export function constrainedCash(cash) {
  return multiplyDecimals(cash, '0.05');
}

export function planningEditor(controls) {
  const values = planningControls(controls);
  return {
    kind: 'planning',
    field: 0,
    error: null,
    draft: Object.fromEntries(PLANNING_FIELDS.map(({ key }) => [key, String(values[key])])),
  };
}
