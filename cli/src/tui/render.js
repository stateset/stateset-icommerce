import { explorerBody, explorerDetail } from './explorer-render.js';
import { VIEWS, records, pageKey, paletteCommands, PRESETS, simulationPlans } from './model.js';
import { editorWindow } from './editor.js';
import { terminalLayout } from './layout.js';
import { clean, fit, width, wrap } from './text.js';
import { DEFAULT_PLANNING, planningFields } from './planning.js';
import { PRODUCT_FIELDS } from './basket.js';
import {
  cashDelta,
  cashMovement,
  cashMovementDelta,
  cashBudgetRefusals,
  purchasingExceptions,
  comparisonEvidence,
  purchasingRulesMatch,
  fillRate,
  soldTraces,
  timelineEntries,
} from './report.js';

const COLORS = { ink: 252, muted: 244, accent: 81, good: 114, warn: 222, bad: 210 };
const money = (record, field, currency = record?.currency) =>
  typeof currency === 'string' && currency.trim() && typeof record?.[field] === 'string'
    ? `${currency} ${record[field]}`
    : 'Unavailable';
const shortId = (value) =>
  width(value) > 15 ? `${clean(value).slice(0, 6)}…${clean(value).slice(-7)}` : value;
const count = (value) => (Number.isSafeInteger(value) && value >= 0 ? value : 'Unavailable');

function detailLines(record, size) {
  if (!record) return ['No record selected.'];
  const lines = [];
  for (const [key, value] of Object.entries(record)) {
    if (Object.hasOwn(record, `${key}Exact`)) continue;
    if (value === null || value === undefined) continue;
    const text =
      typeof value === 'object'
        ? JSON.stringify(
            value,
            (k, v) =>
              typeof v === 'object' && v !== null && !Array.isArray(v)
                ? Object.fromEntries(
                    Object.entries(v).filter(([f]) => !Object.hasOwn(v, `${f}Exact`)),
                  )
                : v,
            2,
          )
        : String(value);
    for (const line of `${key}: ${text}`.split('\n')) lines.push(...wrap(line, size));
    lines.push('');
  }
  return lines;
}

export function detailLimit(state, columns = 80, rows = 24) {
  const { size, available } = terminalLayout(columns, rows);
  const visible = Math.max(1, available - 1);
  const record =
    state.view === 6
      ? explorerDetail(state)
      : state.view === 5
        ? state.sim.result
        : state.focusRecord || records(state)[state.selected];
  return Math.max(0, detailLines(record, size).length - visible);
}

export function resultLines(state, size) {
  const sim = state.sim,
    reports = sim.result?.results || [],
    baseline = reports[0],
    lines = [];
  const add = (line = '') => lines.push(...wrap(line, size));
  if (sim.error) {
    add(`Run failed: ${sim.error}`);
    add('Previous result retained.');
  }
  add(`RESULTS  ${sim.preset} / seed ${sim.seed}`);
  add(
    '↑ ↓ / PgUp PgDn scroll · Home End · t events · v raw · x export · a use assumptions · Esc lab',
  );
  for (const report of reports) {
    const s = report.summary;
    add(`${report.name}: fill ${fillRate(s)} · lost ${s.lostSales} units`);
    add(`Opening cash ${money(s, 'startingCash', report.currency)}`);
    add(`Ending cash ${money(s, 'endingCash', report.currency)}`);
    const movement = cashMovement(report);
    add(`Cash movement ${movement === null ? 'Unavailable' : `${report.currency} ${movement}`}`);
    const low = report.cashFlow;
    const day = low?.minimumCashAt?.day;
    add(
      `Lowest cash ${money(low, 'minimumCash', report.currency)}${typeof low?.minimumCash === 'string' && Number.isSafeInteger(day) && day >= 0 ? ` · day ${day}` : ''}`,
    );
    add(
      `Purchasing ${money(s, 'purchasingSpend', report.currency)} · gross margin ${money(s, 'grossMargin', report.currency)}`,
    );
    add(
      `Inventory at cost: on hand ${money(s, 'endingInventoryValue', report.currency)} · in transit ${money(s, 'inTransitInventoryValue', report.currency)}`,
    );
  }
  add(
    'Cash movement is ending minus opening cash, not profit. Purchases can turn cash into stock.',
  );
  if (reports.length === 2) {
    add(`Change vs baseline: ending cash ${cashDelta(reports[1], baseline, 'endingCash')}`);
    add(
      `                   cash movement ${cashMovementDelta(reports[1], baseline) ?? 'Unavailable (matching currency, horizon and cash evidence required)'}`,
    );
    add(`                   gross margin ${cashDelta(reports[1], baseline, 'grossMargin')}`);
    add('COMPARISON CHECKS');
    const policyMatch = purchasingRulesMatch(reports);
    add(
      `Reorder, sizing & review rules ${policyMatch === null ? 'Unavailable' : policyMatch ? 'same' : 'different'}`,
    );
    const context = comparisonEvidence(reports);
    if (!context) add("Saved comparison evidence is unavailable. Inspect each plan's inputs.");
    else {
      const match = (value) => (value ? 'same' : 'different');
      add(`Opening cash ${match(context.sameOpeningCash)} · horizon ${match(context.sameHorizon)}`);
      add(
        `Opening stock ${match(context.sameOpeningInventory)} · prices/costs ${match(context.sameUnitEconomics)}`,
      );
      add(
        `Purchase priority ${match(context.sameProductPriority)} · cash reserve ${match(context.sameCashReserve)}`,
      );
      add(
        `Projected paths: demand ${match(context.sameDemandPaths)} · supply ${match(context.sameSupplyPaths)}`,
      );
      add('Path matches describe this run; they do not establish equal assumptions.');
    }
  }
  const traces = soldTraces(reports, Math.min(24, Math.max(1, size - 24)));
  add(`SOLD UNITS · up to ${traces.bucket}d/cell · shared peak ${traces.peak}`);
  reports.forEach((r, i) => add(`${r.name}: ${traces.lines[i]}`));
  for (const report of reports) {
    add();
    add(`${report.name} · SKU outcomes`);
    for (const item of report.inventory || []) {
      add(
        `${item.sku}: sold ${item.sold}, lost ${item.lostSales}, available ${item.available}, on order ${item.onOrder}`,
      );
      const count = (value) => (Number.isSafeInteger(value) && value >= 0 ? value : 'n/a');
      add(
        `Purchased ${count(item.purchasedUnits)} · received ${count(item.receivedUnits)} · stockout days ${count(item.stockoutDays)}`,
      );
    }
    add();
    add(`${report.name} · CASH-BUDGET REFUSALS`);
    const refusals = cashBudgetRefusals(report);
    if (refusals === null) add('Timeline unavailable; refusal counts are unknown.');
    else if (!refusals.length) add('No cash-budget refusals recorded.');
    else {
      for (const refusal of refusals) {
        const day = refusal.first.day;
        add(`${refusal.sku}: ${refusal.attempts} blocked attempt(s)`);
        add(
          `First refusal${Number.isSafeInteger(day) && day >= 0 ? ` day ${day}` : ''}: whole order ${money(refusal.first, 'required', report.currency)}; cash ${money(refusal.first, 'cash', report.currency)}`,
        );
      }
      add('Attempts may repeat each day, including day zero; they are not distinct orders.');
    }
    add();
    add(`${report.name} · PURCHASE CONSTRAINTS & REVIEWS`);
    const exceptions = purchasingExceptions(report);
    if (exceptions === null) add('Timeline unavailable; constraint and review counts are unknown.');
    else if (!exceptions.length)
      add('No capacity blocks, reserve blocks or review deferrals recorded.');
    else {
      for (const entry of exceptions) {
        const event = entry.first;
        const day = count(event.day);
        if (event.type === 'purchase_review_deferred') {
          add(`${entry.sku}: ${entry.attempts} deferred review attempt(s)`);
          add(`First deferral day ${day}: inventory position ${count(event.inventoryPosition)}`);
          add(
            `Review interval ${count(event.reviewIntervalDays)} day(s); next review day ${count(event.nextReviewDay)}`,
          );
          if (
            Number.isSafeInteger(event.nextReviewDay) &&
            Number.isSafeInteger(report.days) &&
            event.nextReviewDay > report.days
          )
            add("Next review is outside this report's horizon.");
          add('A scheduled review does not guarantee a purchase.');
          continue;
        }
        const reserve = event.type === 'purchase_reserve_blocked';
        const reason = reserve
          ? 'reserve'
          : event.reason === 'maximum_inventory_position'
            ? 'capacity'
            : 'constraint';
        add(`${entry.sku}: ${entry.attempts} ${reason}-blocked attempt(s)`);
        add(
          `First block day ${day}: whole order ${money(event, 'required', report.currency)}; cash ${money(event, 'cash', report.currency)}`,
        );
        if (reserve)
          add(
            `Reserved cash ${money(event, 'cashReserve', report.currency)}; spendable cash ${money(event, 'spendableCash', report.currency)}`,
          );
        else if (event.reason === 'maximum_inventory_position')
          add(
            `Inventory position with proposed order ${count(event.requiredPosition)}; capacity limit ${count(event.maximumInventoryPosition)}`,
          );
        else add(`Recorded constraint: ${event.reason ?? 'Unavailable'}`);
        add(
          `Target shortfall ${count(event.requestedQuantity)}; rounded order ${count(event.quantity)}; extra units ${count(event.extraUnits)}`,
        );
        add(
          `Supplier minimum ${count(event.minimumOrderQuantity)}; case pack ${count(event.orderMultiple)}`,
        );
      }
      add(
        'Counts are recorded attempts, not distinct orders. t shows every event and later outcome.',
      );
    }
  }
  add();
  add('EXECUTED ASSUMPTIONS');
  const inputs = sim.result?.inputs;
  if (!Array.isArray(inputs) || !inputs.length) add('Saved input plans are unavailable.');
  for (const [planIndex, plan] of (Array.isArray(inputs) ? inputs : []).entries()) {
    add(
      `${plan.name || reports[planIndex]?.name || `Plan ${planIndex + 1}`} · ${plan.days ?? 'Unavailable'} days · ${money(plan, 'startingCash', plan.currency || reports[planIndex]?.currency)} opening cash`,
    );
    if (plan.cashReserve !== undefined)
      add(
        `Cash reserve ${money(plan, 'cashReserve', plan.currency || reports[planIndex]?.currency)}`,
      );
    for (const [index, item] of (plan.products || []).entries()) {
      add(
        `Priority ${index + 1} · ${item.sku}: stock ${item.initialStock}, price ${item.unitPrice}, cost ${item.unitCost}`,
      );
      add(
        `Demand ${item.demand.min}–${item.demand.max}/day · reorder ${item.reorderPoint} → ${item.targetStock} · lead ${item.leadTimeDays}d`,
      );
      if (item.purchasing)
        add(
          `Supplier minimum ${item.purchasing.minimumOrderQuantity} · case pack ${item.purchasing.orderMultiple}`,
        );
      add(`Purchase review interval ${count(item.purchasing?.reviewIntervalDays ?? 1)} day(s)`);
      if (item.purchasing?.maximumInventoryPosition !== undefined)
        add(`Maximum inventory position ${count(item.purchasing.maximumInventoryPosition)}`);
    }
  }
  add('Cash is shared across SKUs; product input order sets purchase priority.');
  add('Conditional projections, not validated forecasts. Esc returns to the lab.');
  return lines;
}

const EVENT_MONEY = new Set(['amount', 'required', 'cash', 'cashReserve', 'spendableCash']);
const EVENT_LABELS = {
  cash: 'Cash after event',
  amount: 'Purchase spend',
  required: 'Whole order required',
  quantity: 'Units',
  requestedQuantity: 'Target shortfall',
  extraUnits: 'Extra units from supplier terms',
  arrivalDay: 'Expected arrival day',
  nextArrivalDay: 'Next arrival day',
  remaining: 'Remaining units',
  firstReceiptBps: 'First receipt (basis points)',
};
export function timelineLines(state, size) {
  const lines = [],
    add = (line = '') => lines.push(...wrap(line, size));
  const query = state.input?.kind === 'search' ? state.input.text : state.panelQuery || '';
  add(`EVENT TIMELINE · ${state.sim.preset} / seed ${state.sim.seed}`);
  add('Recorded order · / SKU or event · ↑ ↓ Home End · v raw · Esc results');
  if (query) add(`Filter: ${query}`);
  const reports = state.sim.result?.results;
  if (!Array.isArray(reports) || !reports.length) add('Saved reports unavailable.');
  for (const report of reports || []) {
    add();
    const selected = timelineEntries(report, query);
    add(
      `${report.name || 'Unnamed plan'} · ${selected ? `${selected.entries.length}/${selected.total} recorded events` : 'timeline unavailable'}`,
    );
    if (!selected) continue;
    if (!selected.total) add('No recorded events.');
    else if (!selected.entries.length) add('No events match. Esc clears the filter.');
    for (const { event, index } of selected.entries) {
      if (!event || typeof event !== 'object' || Array.isArray(event)) {
        add(`${index + 1}. Invalid recorded event. v opens the raw report.`);
        continue;
      }
      const day = Number.isSafeInteger(event.day) && event.day >= 0 ? event.day : 'unavailable';
      const type = typeof event.type === 'string' ? event.type.replace(/_/g, ' ') : 'unknown event';
      add(`${index + 1}. Day ${day} · ${type} · ${event.sku ?? 'SKU unavailable'}`);
      for (const [key, value] of Object.entries(event)) {
        if (['day', 'type', 'sku'].includes(key)) continue;
        const label = Object.hasOwn(EVENT_LABELS, key)
          ? EVENT_LABELS[key]
          : key.replace(/([a-z])([A-Z])/g, '$1 $2');
        const text = EVENT_MONEY.has(key)
          ? money(event, key, report.currency)
          : value === null || value === undefined
            ? 'Unavailable'
            : typeof value === 'boolean'
              ? value
                ? 'Yes'
                : 'No'
              : typeof value === 'object'
                ? JSON.stringify(value)
                : String(value);
        add(`${label}: ${text}`);
      }
    }
  }
  add('Filtered view only; saved reports and exports retain every recorded event.');
  return lines;
}
export function importReviewLines(preview, size) {
  const { settings } = preview,
    lines = [];
  const add = (line) => lines.push(...wrap(line, size));
  add('IMPORT SCENARIO · REVIEW ASSUMPTIONS');
  add(`${preview.filename} · exported by CLI ${preview.cliVersion}`);
  add('File-provided results are discarded; digest is not a signature.');
  add(`${PRESETS.find((p) => p.id === settings.presetId)?.name} · seed ${settings.seed}`);
  add(`${settings.controls.days} days · USD ${settings.controls.startingCash} opening cash`);
  add(`Cash reserve USD ${settings.controls.cashReserve ?? '0.00'} · shared across SKUs`);
  const products = settings.products.length
    ? settings.products
    : [
        {
          sku: 'COFFEE-250G (demo recipe)',
          initialStock: settings.controls.initialStock,
          unitPrice: '12.50',
          unitCost: '5.25',
          demand: { min: 2, max: 5 },
          reorderPoint: settings.controls.reorderPoint,
          targetStock: settings.controls.targetStock,
          leadTimeDays: settings.controls.leadTimeDays,
        },
      ];
  for (const p of products) {
    add(`${p.sku}: stock ${p.initialStock}, price ${p.unitPrice}, cost ${p.unitCost}`);
    add(
      `Demand ${p.demand.min}–${p.demand.max}/day · reorder ${p.reorderPoint} → ${p.targetStock} · lead ${p.leadTimeDays}d`,
    );
    if (p.purchasing)
      add(
        `Supplier minimum ${p.purchasing.minimumOrderQuantity} · case pack ${p.purchasing.orderMultiple}`,
      );
    add(`Purchase review interval ${p.purchasing?.reviewIntervalDays ?? 1} day(s)`);
    add(`Maximum inventory position ${p.purchasing?.maximumInventoryPosition ?? 'No cap'}`);
  }
  add('Enter loads assumptions only.');
  add('Run and save are separate actions.');
  return lines;
}

export function importReviewLimit(preview, columns = 80, rows = 24) {
  const { size, available } = terminalLayout(columns, rows);
  const visible = Math.max(1, available - 1);
  return Math.max(0, importReviewLines(preview, size).length - visible);
}

export function scenarioUpdateLines(review, size) {
  const lines = [],
    add = (line) => lines.push(...wrap(line, size));
  add(`Target: ${review.expected.name}`);
  for (const [label, settings] of [
    ['SAVED', review.expected],
    ['PROPOSED', review.settings],
  ]) {
    const preset = PRESETS.find((p) => p.id === settings.presetId);
    add(`${label} · ${preset.name} · seed ${settings.seed}`);
    add(preset.note);
    // Render the exact baseline plan, including the default recipe when no SKU is selected.
    const plan = simulationPlans(
      settings.presetId,
      settings.seed,
      settings.controls,
      settings.products,
    )[0];
    add(`${plan.days} days · USD ${plan.startingCash} opening cash`);
    add(`Cash reserve USD ${plan.cashReserve ?? '0.00'} · shared across SKUs`);
    for (const [index, item] of plan.products.entries()) {
      add(
        `Priority ${index + 1} · ${item.sku}: stock ${item.initialStock}, price ${item.unitPrice}, cost ${item.unitCost}`,
      );
      add(
        `Demand ${item.demand.min}–${item.demand.max}/day · reorder ${item.reorderPoint} → ${item.targetStock} · lead ${item.leadTimeDays}d`,
      );
      if (item.purchasing)
        add(
          `Supplier minimum ${item.purchasing.minimumOrderQuantity} · case pack ${item.purchasing.orderMultiple}`,
        );
      add(`Purchase review interval ${item.purchasing?.reviewIntervalDays ?? 1} day(s)`);
      add(`Maximum inventory position ${item.purchasing?.maximumInventoryPosition ?? 'No cap'}`);
    }
  }
  add('Enter replaces this saved scenario. ID, name and run reports are preserved.');
  add('No simulation runs. w still saves a separate scenario.');
  return lines;
}

export function scenarioUpdateLimit(review, columns = 80, rows = 24) {
  const { size, available } = terminalLayout(columns, rows);
  const visible = Math.max(1, available - 1);
  return Math.max(0, scenarioUpdateLines(review, size).length - visible);
}

export function panelLimit(state, columns = 80, rows = 24) {
  const { size, available } = terminalLayout(columns, rows);
  const lines = state.panel === 'timeline' ? timelineLines(state, size) : resultLines(state, size);
  return Math.max(0, lines.length - Math.max(1, available - 1));
}

function tableRow(view, row, size) {
  if (view === 1)
    return `${fit(shortId(row.orderNumber || row.id), 15)} ${fit(row.status, 13)} ${money(row, 'totalAmountExact')}`;
  if (view === 2)
    return `${fit(row.sku, Math.min(20, Math.floor(size / 3)))} ${fit(row.available ?? '—', 10)} ${row.name}`;
  if (view === 3)
    return `${fit(`${row.firstName} ${row.lastName}`, Math.min(24, Math.floor(size / 2)))} ${row.email}`;
  if (view === 4)
    return `${fit(shortId(row.paymentNumber || row.id), 15)} ${fit(row.status, 13)} ${money(row, 'amountExact')}`;
  return `${fit(row.name, 23)} ${row.note}`;
}

function staticBody(state, compact = false) {
  const body = [];
  const put = (text = '', tone = 'ink', bold = false) => body.push({ text, tone, bold });
  if (compact && !state.help) {
    const counts = state.data?.counts || {};
    put('COMMERCE', 'accent', true);
    for (const field of ['orders', 'customers', 'products', 'payments'])
      put(`${field}: ${counts[field] ?? '—'}`);
    put(`Stock alerts: ${state.data?.inventoryCount ?? '—'}`);
    for (const item of state.data?.alerts || []) put(`${item.sku}: ${item.available} available`);
    put('6 Simulation lab');
    put('7 Engine explorer');
    put('1–7 views · : commands');
    return body;
  }
  if (state.help) {
    put('KEYBOARD HELP · ↑ ↓ scroll · Esc close', 'accent', true);
    for (const line of [
      '1–7 / Tab       Switch view',
      '7               Engine explorer: every domain and tool reference',
      'Explorer d / T  Domains / tool reference (documentation only)',
      'Explorer [ / ]  Previous / next visible domain group',
      'Explorer t / b  Follow persisted links / return',
      'Explorer /      Filter domains or search the entire resource on Enter',
      '↑ ↓ / j k       Select a record or scenario',
      'Enter           Open details / run selected simulation',
      'Esc             Back / clear search',
      '/               Type preview; Enter searches entire view',
      ': / Ctrl+P      Command palette',
      'n / p           Next / previous data page',
      'r               Refresh the store snapshot',
      'o               Toggle recent / alphabetical sort',
      'a               Add selected inventory SKU to a plan',
      't / u / b       Related records / customer / back',
      'f               Scrollable comparison results',
      't (lab/result)  Recorded event timeline · / filter SKU or event',
      'w / l / h       Save scenario / library / run history',
      'x / i / c       Export result / import assumptions / cancel run',
      '/ in library    Filter saved entries without reading the store',
      'Shift+N         Rename selected saved scenario',
      'Shift+S         Review current draft as an update to selected saved scenario',
      'Shift+D / U     Remove saved entry / undo last removal',
      'p (lab)         Edit selected SKU planning terms',
      '[ / ] (SKUs)    Move selected SKU earlier / later in purchase priority',
      's               Run selected simulation in the lab',
      'e               Edit planning assumptions',
      'g               Set a seed directly (1–999999)',
      '← → Home End    Move within an input field',
      'Delete / Ctrl+K Delete next character / remaining text',
      'Ctrl+U          Clear the input field',
      'Shift+R         Replay last successful simulation inputs',
      'a (run/history) Load original run assumptions into an editable draft',
      '+ / -           Change the lab seed',
      'v               Inspect the full simulation report',
      'Home / End      Jump to the start / end of records or a report',
      '?               Toggle this help',
      'q / Ctrl+C      Quit and restore the terminal',
      '',
      'Store browsing has no business-write actions.',
      'Lab runs use fresh private engine databases.',
      'Projections reflect assumptions; they are not validated forecasts.',
    ])
      put(line, line ? 'ink' : 'muted');
  } else {
    const counts = state.data?.counts || {},
      health = state.data?.health;
    put('COMMERCE AT A GLANCE', 'muted');
    put(
      `${fit(counts.orders ?? '—', 12)}${fit(counts.customers ?? '—', 12)}${fit(counts.products ?? '—', 12)}${counts.payments ?? '—'}`,
      'accent',
      true,
    );
    put(`${fit('ORDERS', 12)}${fit('CUSTOMERS', 12)}${fit('PRODUCTS', 12)}PAYMENTS`, 'muted');
    put();
    put('STOCK WATCH', 'muted');
    put(
      `${health?.totalSkus ?? '—'} tracked SKUs   ·   ${state.data?.inventoryCount ?? '—'} low-stock signals`,
      state.data?.inventoryCount > 0 ? 'warn' : 'muted',
    );
    put();
    put('NEEDS ATTENTION', 'muted');
    const inventory = state.data?.alerts ?? state.data?.inventory;
    if (inventory?.length)
      inventory
        .slice(0, 4)
        .forEach((item) =>
          put(
            `${fit(item.sku, 20)} ${item.available} available / reorder ${item.reorderPoint ?? '—'}`,
            'warn',
          ),
        );
    else
      put(
        inventory ? 'No low-stock items reported by the engine.' : 'Stock signals are unavailable.',
        inventory ? 'good' : 'muted',
      );
    put();
    put('EXPLORE THE POSSIBILITIES', 'muted');
    put('7  Explore all engine domains and tool references', 'accent');
    put('6  Open the simulation lab', 'accent');
    put('Compare demand, supplier delays and cash constraints.');
    put('Every scenario runs in a separate embedded store.', 'muted');
    put();
    put('RECENT ORDERS', 'muted');
    for (const order of (state.data?.recentOrders || state.data?.orders || []).slice(0, 3))
      put(
        `${shortId(order.orderNumber || order.id)}  ${order.status}  ${money(order, 'totalAmountExact')}`,
      );
  }
  return body;
}
function staticLines(state, size, compact) {
  const lines = staticBody(state, compact);
  return compact
    ? lines.flatMap((item) =>
        wrap(item.text.replace(/\s{2,}/g, ' '), size).map((text) => ({ ...item, text })),
      )
    : lines;
}
function compactFooter(state, w) {
  const input = state.input;
  if (input?.saving) return ' Saving… please wait';
  if (input?.kind === 'scenario-update') return ' Enter replace Esc';
  if (input?.kind === 'import-review') return ' Enter load Esc';
  if (['planning', 'product'].includes(input?.kind)) return ' Tab next ^S Esc';
  if (input) return ` ${editorWindow(input.text, input.cursor, w - 2)}`;
  if (state.help || state.detail || state.panel === 'results') return ' ↑↓ Home End Esc';
  if (state.panel === 'timeline') return ' ↑↓ / filter Esc';
  if (state.sim.running) return ' c cancel · : ?';
  if (state.panel) return ' Enter / : ? Esc';
  if (state.view === 5) return ' s run e edit : ?';
  return ' 1-7 / Enter : ?';
}
export function contentLimit(state, _columns = 80, rows = 24) {
  const { size, available, compact } = terminalLayout(_columns, rows);
  return Math.max(0, staticLines(state, size, compact).length - available);
}

export function render(state, columns = 80, rows = 24, { color = true } = {}) {
  const { w, h, compact, sidebar, size, available, usable } = terminalLayout(columns, rows);
  const style = (value, tone = 'ink', bold = false) =>
    color ? `\x1b[${bold ? '1;' : ''}38;5;${COLORS[tone]}m${value}\x1b[0m` : value;
  const out = [];
  const add = (value = '', tone = 'ink', bold = false) =>
    out.push(style(fit(value, w), tone, bold));
  if (!usable) {
    add(' STATESET', 'accent', true);
    add(' Need 20 × 8', 'warn');
    add(' q Quit');
    return out.slice(0, h).join('\n');
  }
  if (compact) {
    add(
      ` ${state.view + 1} ${['Home', 'Orders', 'Stock', 'People', 'Pay', 'Lab', 'Explore'][state.view]} · ${state.data ? 'ready' : state.loading ? 'loading' : 'offline'}`,
      'accent',
      true,
    );
    add(` ${state.source}`, 'muted');
  } else {
    add(' STATESET  /  iCommerce', 'accent', true);
    add(
      ` ${state.source}  ·  ${state.data ? 'Engine connected' : state.loading ? 'Connecting…' : 'Not connected'}`,
      'muted',
    );
    add(
      ' ' +
        VIEWS.map((v, i) =>
          w < 80 && i !== state.view
            ? `${i + 1}`
            : `${i + 1} ${['Home', 'Orders', 'Stock', 'People', 'Pay', 'Lab', 'Explore'][i]}`,
        ).join('  '),
      'muted',
    );
    add(
      ` ${VIEWS[state.view]}${(state.input?.kind === 'search' ? state.input.text : state.view === 6 ? state.explorer.query : state.query) ? `  / ${state.input?.kind === 'search' ? state.input.text : state.view === 6 ? state.explorer.query : state.query}` : ''}${state.loading ? '  ·  Refreshing…' : ''}`,
      'accent',
      true,
    );
  }
  const body = [];
  const put = (text = '', tone = 'ink', bold = false) => body.push({ text, tone, bold });
  if (['import-review', 'scenario-update'].includes(state.input?.kind)) {
    const update = state.input.kind === 'scenario-update';
    const lines = update
        ? scenarioUpdateLines(state.input, size)
        : importReviewLines(state.input.preview, size),
      visible = Math.max(1, available - 1);
    const offset = Math.min(state.input.offset, Math.max(0, lines.length - visible));
    put(
      compact
        ? `${update ? 'UPDATE' : 'IMPORT'} ${offset + 1}/${lines.length}`
        : `${update ? 'UPDATE SCENARIO' : 'IMPORT PREVIEW'} · ${offset + 1}–${Math.min(lines.length, offset + visible)}/${lines.length} · ↑ ↓ scroll`,
      'accent',
      true,
    );
    lines.slice(offset, offset + visible).forEach((line) => put(line));
  } else if (state.input?.kind === 'planning' || state.input?.kind === 'product') {
    const editor = state.input;
    const fields = editor.kind === 'product' ? PRODUCT_FIELDS : planningFields(state.products);
    if (compact) {
      const field = fields[editor.field];
      put(
        `${editor.kind === 'product' ? 'SKU' : 'PLAN'} ${editor.field + 1}/${fields.length}`,
        'accent',
        true,
      );
      put(field.label);
      put(editorWindow(editor.draft[field.key], editor.cursor, size), 'accent', true);
      wrap(field.hint, size)
        .slice(0, Math.max(0, available - 3))
        .forEach((line) => put(line, 'muted'));
    } else {
      put(
        editor.kind === 'product'
          ? `SKU TERMS · ${editor.sku} · assumed USD`
          : 'PLANNING ASSUMPTIONS  ·  private scenarios only',
        'accent',
        true,
      );
      put('← → edit · Home/End · Tab next · Ctrl+S save · Esc cancel', 'muted');
      const visible = Math.max(1, available - 4),
        start = Math.max(0, editor.field - visible + 1);
      for (const [index, field] of fields.slice(start, start + visible).entries()) {
        const active = start + index === editor.field;
        put(
          `${active ? '›' : ' '} ${fit(field.label, Math.min(23, size - 16))} [${active ? editorWindow(editor.draft[field.key], editor.cursor, Math.max(1, size - Math.min(23, size - 16) - 6)) : editor.draft[field.key]}]`,
          active ? 'accent' : 'ink',
          active,
        );
        if (active) put(`  ${field.hint}`, 'muted');
      }
      if (editor.error) put(editor.error, 'bad');
      else put('Changes affect the next run. Existing results keep their inputs.', 'muted');
    }
  } else if (
    ['results', 'timeline'].includes(state.panel) &&
    !state.detail &&
    !state.help &&
    (!state.input || (state.panel === 'timeline' && state.input.kind === 'search'))
  ) {
    const timeline = state.panel === 'timeline';
    const lines = timeline ? timelineLines(state, size) : resultLines(state, size),
      visible = Math.max(1, available - 1);
    const offset = Math.min(state.panelOffset, Math.max(0, lines.length - visible));
    put(
      compact
        ? `${timeline ? 'EVENTS' : 'RESULTS'} ${offset + 1}/${lines.length}`
        : `${timeline ? 'EVENT TIMELINE' : 'COMPARISON'} · ${offset + 1}–${Math.min(lines.length, offset + visible)} / ${lines.length} lines · ${timeline ? '/ filter · Esc results' : 'Esc lab'}`,
      'muted',
    );
    lines.slice(offset, offset + visible).forEach((line) => put(line));
  } else if (
    ['scenarios', 'history', 'products'].includes(state.panel) &&
    !state.input &&
    !state.help
  ) {
    const rows = records(state);
    put(
      {
        scenarios: compact
          ? 'SCENARIOS'
          : 'SAVED SCENARIOS · Enter load · N rename · S update · / filter',
        history: compact ? 'HISTORY' : 'RUN HISTORY · Enter inspect · a use · / filter',
        products: compact ? 'SKU PRIORITY' : 'SELECTED SKUS · PURCHASE PRIORITY',
      }[state.panel],
      'accent',
      true,
    );
    const allRows = {
      scenarios: state.library.scenarios,
      history: state.library.runs,
      products: state.products,
    }[state.panel];
    if (!compact && state.panel === 'products')
      put('[ earlier · ] later · Enter edit · / filter', 'muted');
    put(
      compact
        ? `${rows.length}/${allRows.length}${state.panelQuery ? ` / ${state.panelQuery}` : ''}`
        : `${rows.length} / ${allRows.length} entries${state.panelQuery ? ` · / ${state.panelQuery}` : ''} · ${state.panel === 'products' ? 'd' : 'Shift+D'} remove${state.undoRemoval ? ' · Shift+U undo' : ''}`,
      'muted',
    );
    const count = Math.max(
      1,
      compact ? available - 2 : Math.floor((available - (state.panel === 'products' ? 3 : 2)) / 2),
    );
    const start = Math.max(0, state.selected - count + 1);
    rows.slice(start, start + count).forEach((item, i) => {
      put(
        `${start + i === state.selected ? '›' : ' '} ${state.panel === 'products' ? `${allRows.indexOf(item) + 1}. ` : ''}${item.name || item.sku || item.preset}`,
        start + i === state.selected ? 'accent' : 'ink',
      );
      if (!compact)
        put(
          state.panel === 'products'
            ? `  cost ${item.unitCost} · demand ${item.demand.min}–${item.demand.max} · lead ${item.leadTimeDays}d`
            : `  ${item.controls.days}d · USD ${item.controls.startingCash} · seed ${item.seed} · ${item.products.length || 1} SKU(s)${item.createdAt ? ' · ' + item.createdAt.slice(0, 10) : ''}`,
          'muted',
        );
    });
    if (!rows.length)
      put(
        state.panel === 'products'
          ? state.panelQuery
            ? 'No SKUs match this filter. Esc clears it.'
            : 'Open Stock (3), select a SKU, then a to add.'
          : 'No saved entries yet. w saves a scenario; successful runs save history.',
        'muted',
      );
  } else if (['save', 'rename', 'export', 'import', 'seed'].includes(state.input?.kind)) {
    put(
      state.input.kind === 'seed'
        ? 'SET SIMULATION SEED · 1–999999'
        : state.input.kind === 'save'
          ? 'SAVE SCENARIO · enter a name'
          : state.input.kind === 'rename'
            ? 'RENAME SCENARIO · 1–80 characters'
            : state.input.kind === 'import'
              ? 'IMPORT SCENARIO · enter a local JSON filename'
              : 'EXPORT RUN · enter a new JSON filename',
      'accent',
      true,
    );
    put(
      state.input.saving
        ? 'Renaming… please wait'
        : compact
          ? 'Enter / Esc cancel'
          : 'Enter confirm · Esc cancel',
      'muted',
    );
    put(
      state.input.kind === 'seed'
        ? 'Enter saves a valid seed. Existing results keep their original seed.'
        : state.input.kind === 'save'
          ? `Saves ${state.selectedPreset} with current seed, assumptions and SKUs.`
          : state.input.kind === 'rename'
            ? 'Changes only the saved name. Assumptions, priority and reports stay intact.'
            : state.input.kind === 'import'
              ? 'Validate and preview exported assumptions. Nothing runs automatically.'
              : 'Includes executed inputs, input digest and the complete engine report.',
    );
    put(
      state.input.kind === 'seed'
        ? 'Use s to run the selected scenario.'
        : state.input.kind === 'rename'
          ? 'The renamed entry remains selected; the library filter is cleared.'
          : 'Existing files and saved entries are preserved.',
      'muted',
    );
  } else if (state.input?.kind === 'palette') {
    put(compact ? 'COMMANDS' : 'COMMAND PALETTE', 'accent', true);
    if (!compact) put('↑ ↓ select  ·  Enter open  ·  Type to filter  ·  Esc dismiss', 'muted');
    const commands = paletteCommands(state.input.text, state),
      selected = state.input.selected || 0;
    const count = Math.max(1, available - (compact ? 1 : 2));
    const start = Math.max(0, selected - count + 1);
    commands
      .slice(start, start + count)
      .forEach((c, i) =>
        put(
          `${i + start === selected ? '› ' : '  '}${c.label}`,
          i + start === selected ? 'accent' : 'ink',
        ),
      );
    if (!commands.length) put('No matching commands.', 'muted');
  } else if (state.help) {
    body.push(...staticLines(state, size, compact));
  } else if (!state.data && state.error && state.view !== 5 && state.view !== 6) {
    put('THE ENGINE COULD NOT OPEN', 'bad', true);
    put();
    wrap(state.error, size).forEach((line) => put(line, 'warn'));
    put();
    put('r  Retry loading the store', 'accent');
    put('?  Keyboard help');
    put('q  Quit');
    put();
    put('No records have been loaded. The source store is unchanged.', 'muted');
  } else if (state.view === 0) {
    body.push(...staticLines(state, size, compact));
  } else if (state.detail) {
    const lines = detailLines(
      state.view === 6
        ? explorerDetail(state)
        : state.view === 5
          ? state.sim.result
          : state.focusRecord || records(state)[state.selected],
      size,
    );
    const offset = Math.min(state.detailOffset, Math.max(0, lines.length - available + 1));
    put(
      compact
        ? 'DETAILS · ↑↓ Esc'
        : state.view === 6
          ? state.explorer.mode === 'tools'
            ? 'TOOL REFERENCE · ↑ ↓ scroll · Esc close'
            : 'STORED RECORD · ↑ ↓ scroll · t links · Esc close'
          : state.view === 5
            ? 'RECORD DETAILS · ↑ ↓ scroll · Esc back'
            : 'DETAILS · t related · u customer · b back · Esc close',
      'muted',
    );
    lines.slice(offset, offset + available - 1).forEach((line) => put(line));
  } else if (state.view === 6) {
    body.push(...explorerBody(state, size, available, compact));
  } else {
    const data = records(state),
      field = pageKey(state.view);
    if (state.view === 5) {
      if (compact) {
        put(`Seed ${state.seed} · ${state.planning.days}d`, 'good');
        put(`USD ${state.planning.startingCash} · ${state.products.length || 1} SKU`, 'muted');
      } else {
        put(`ISOLATED ENGINE  ·  Seed ${state.seed}  ·  g set · + / -`, 'good');
        put(
          `${state.planning.days} days · USD ${state.planning.startingCash} cash · e edit assumptions`,
          'muted',
        );
        if (state.planning.cashReserve && state.planning.cashReserve !== '0.00')
          put(`USD ${state.planning.cashReserve} reserve · shared across SKUs`, 'muted');
        put(
          `${state.products.length || 1} SKU(s) · p selected SKUs · w save · l library · h history`,
          'accent',
        );
        put(
          state.products.length
            ? state.products.map((p) => p.sku).join(' · ')
            : `Demo recipe · Stock ${state.planning.initialStock} · Reorder ${state.planning.reorderPoint} → ${state.planning.targetStock} · Lead ${state.planning.leadTimeDays}d`,
          'muted',
        );
      }
    } else
      put(
        compact
          ? `Page ${(state.data?.pages?.[field] || 0) + 1} · ${data.length} rows`
          : `Page ${(state.data?.pages?.[field] || 0) + 1} · ${data.length} shown / ${state.data?.matched?.[field] ?? state.data?.counts?.[field] ?? '—'} matches · n / p · o sort`,
        'muted',
      );
    if (!compact && state.relation && state.relation.field === field)
      put(`Related ${state.relation.column}: ${state.relation.id} · b back`, 'muted');
    if (!compact && state.view !== 5)
      put(
        [
          '',
          'ORDER           STATUS        TOTAL',
          'SKU                  AVAILABLE  NAME · a plan',
          'CUSTOMER                  EMAIL',
          'PAYMENT         STATUS        AMOUNT',
        ][state.view],
        'muted',
      );
    const listHeight = compact
      ? Math.max(1, available - 2)
      : state.view === 5
        ? Math.min(4, Math.max(1, available - 10))
        : Math.max(1, available - 3);
    const start = Math.max(0, state.selected - listHeight + 1);
    for (const [i, record] of data.slice(start, start + listHeight).entries()) {
      const selected = i + start === state.selected;
      put(
        `${selected ? '›' : ' '} ${compact ? record.orderNumber || record.sku || record.email || record.paymentNumber || record.name || record.id : tableRow(state.view, record, size - 2)}`,
        selected ? 'accent' : 'ink',
        selected,
      );
    }
    if (!data.length)
      put(
        state.data
          ? 'No matching records on this page. Esc clears the filter.'
          : 'Waiting for the embedded engine…',
        'muted',
      );
    if (compact && state.view !== 5 && data[state.selected]) {
      const item = data[state.selected];
      put(
        state.view === 2
          ? `Available ${item.available ?? '—'}`
          : item.status || 'Enter for details',
        'muted',
      );
    }
    if (state.view === 5 && !compact) {
      put();
      const sim = state.sim;
      if (sim.running) put(`RUNNING  ${sim.pending?.preset} / seed ${sim.pending?.seed}`, 'warn');
      if (sim.error) {
        wrap(`Run failed: ${sim.error}`, size)
          .slice(0, 2)
          .forEach((line) => put(line, 'bad'));
        put(sim.result ? 'Previous result retained.' : 'No result available.', 'muted');
      }
      if (sim.result) {
        put(`RESULTS ${sim.preset} / seed ${sim.seed} · f compare · v raw · R replay`, 'good');
        if (
          (sim.presetId && sim.presetId !== state.selectedPreset) ||
          JSON.stringify({ ...DEFAULT_PLANNING, ...sim.controls }) !==
            JSON.stringify({ ...DEFAULT_PLANNING, ...state.planning }) ||
          sim.seed !== state.seed ||
          JSON.stringify(sim.products || []) !== JSON.stringify(state.products)
        )
          put('Assumptions changed since this result. s runs the new settings.', 'warn');
      } else if (!sim.running && !sim.error)
        put('Enter / s run · e assumptions · 3 then a to select stock', 'muted');
      put('Conditional projections from supplied assumptions.', 'muted');
    }
  }
  for (let i = 0; i < available; i++) {
    const offset =
      !state.input && (state.help || state.view === 0)
        ? Math.min(state.contentOffset, Math.max(0, body.length - available))
        : 0;
    const item = body[i + offset] || { text: '', tone: 'ink' };
    if (sidebar) {
      const left =
        i < VIEWS.length
          ? `${i === state.view ? '›' : ' '} ${i + 1} ${VIEWS[i]}`
          : i === 8
            ? 'LOCAL FIRST'
            : i === 9
              ? 'Exact money'
              : i === 10
                ? 'Private simulations'
                : '';
      out.push(
        style(fit(left, 20), i === state.view ? 'accent' : 'muted') +
          style('│ ', 'muted') +
          style(fit(item.text, w - 22), item.tone, item.bold),
      );
    } else add(' ' + item.text, item.tone, item.bold);
  }
  const errors = state.data?.errors || [];
  add(
    ` ${state.input?.error ? state.input.error : state.error ? `STALE / ERROR  ${state.error}` : errors.length ? `PARTIAL DATA  ${errors.map((e) => e.section).join(', ')}` : state.sim.running ? `RUNNING ${Math.floor((Date.now() - state.sim.startedAt) / 1000)}s · ${state.sim.progress || 'Engine working'} · c cancel` : state.notice || (state.sim.error ? `Run failed: ${state.sim.error}` : compact ? (state.sim.result && state.view === 5 ? 'f results · R replay' : '? keyboard help') : state.data ? `Snapshot ${state.data.checkedAt}  ·  r refresh` : 'Loading engine…')}`,
    state.error || errors.length || state.input?.error || state.sim.error ? 'warn' : 'muted',
  );
  add(
    compact
      ? compactFooter(state, w)
      : state.input?.kind === 'scenario-update'
        ? state.input.saving
          ? ' Updating scenario… please wait'
          : ' Enter replace saved scenario  ↑ ↓ scroll  Esc cancel'
        : state.input?.kind === 'import-review'
          ? ' Enter load assumptions  ↑ ↓ scroll  Esc cancel'
          : ['planning', 'product'].includes(state.input?.kind)
            ? ' Ctrl+U clear  Tab next  Ctrl+S save  Esc cancel'
            : state.input
              ? state.input.saving
                ? ' Renaming… please wait'
                : ` ${{ search: '/', palette: ':', save: 'Name: ', rename: 'Name: ', export: 'Path: ', import: 'Path: ', seed: 'Seed:' }[state.input.kind] || ''} ${editorWindow(state.input.text, state.input.cursor, Math.max(1, w - 36))}  Enter accept · Esc cancel`
              : state.view === 6
                ? ' d domains  T tools  / search  Enter open  b back  ? help  q quit'
                : state.view === 5
                  ? ' e edit  p SKUs  l/h library/history  i import  : commands  ? help'
                  : state.view === 2
                    ? ' a plan SKU  / search  o sort  Enter inspect  ? help  q quit'
                    : ' / search  o sort  Enter inspect  : commands  ? help  q quit',
    state.input ? 'accent' : 'muted',
  );
  // Pad each row; stay off the last physical column/row to avoid wrapping and
  // terminal scrolling, including when Herdr resizes a split pane.
  return out
    .slice(0, h)
    .map((line) => line + ' '.repeat(Math.max(0, w - width(line))))
    .join('\n');
}
