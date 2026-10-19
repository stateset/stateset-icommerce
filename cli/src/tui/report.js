import { subtractDecimals } from '../utils/exact-decimal.js';

// Refusals are timeline attempts, including repeated days and day zero.
// Preserve exact recorded money; this is presentation, not purchase sizing.
export function cashBudgetRefusals(report) {
  if (!Array.isArray(report?.timeline)) return null;
  const bySku = new Map();
  for (const event of report.timeline) {
    if (event?.type !== 'purchase_budget_blocked') continue;
    const sku = typeof event.sku === 'string' ? event.sku : '(unknown SKU)';
    const entry = bySku.get(sku) || { sku, attempts: 0, first: event };
    entry.attempts += 1;
    bySku.set(sku, entry);
  }
  return [...bySku.values()];
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
