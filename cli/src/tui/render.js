import { explorerBody, explorerDetail } from './explorer-render.js';
import { VIEWS, records, pageKey, paletteCommands, PRESETS } from './model.js';
import { editorWindow } from './editor.js';
import { clean, fit, width, wrap } from './text.js';
import { PLANNING_FIELDS } from './planning.js';
import { PRODUCT_FIELDS } from './basket.js';
import {
  cashDelta,
  cashBudgetRefusals,
  comparisonEvidence,
  fillRate,
  soldTraces,
} from './report.js';

const COLORS = { ink: 252, muted: 244, accent: 81, good: 114, warn: 222, bad: 210 };
const money = (record, field, currency = record?.currency || 'USD') =>
  typeof record?.[field] === 'string' ? `${currency} ${record[field]}` : 'Unavailable';
const shortId = (value) =>
  width(value) > 15 ? `${clean(value).slice(0, 6)}…${clean(value).slice(-7)}` : value;

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
  const w = Math.max(1, Math.min(220, columns - 1));
  const h = Math.max(1, Math.min(100, rows - 1));
  const size = Math.max(1, w - (w >= 110 ? 22 : 2));
  const visible = Math.max(1, h - 9);
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
  add('↑ ↓ / PgUp PgDn scroll · Home End · v raw · x export · a use assumptions · Esc lab');
  for (const report of reports) {
    const s = report.summary;
    add(`${report.name}: fill ${fillRate(s)} · lost ${s.lostSales} units`);
    add(`Ending cash ${money(s, 'endingCash', report.currency)}`);
    const low = report.cashFlow;
    const day = low?.minimumCashAt?.day;
    add(
      `Lowest cash ${money(low, 'minimumCash', report.currency)}${typeof low?.minimumCash === 'string' && Number.isSafeInteger(day) && day >= 0 ? ` · day ${day}` : ''}`,
    );
    add(
      `Purchasing ${money(s, 'purchasingSpend', report.currency)} · margin ${money(s, 'grossMargin', report.currency)}`,
    );
  }
  if (reports.length === 2) {
    add(`Change vs baseline: cash ${cashDelta(reports[1], baseline, 'endingCash')}`);
    add(`                   margin ${cashDelta(reports[1], baseline, 'grossMargin')}`);
    add('COMPARISON CHECKS');
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
  }
  add();
  add('EXECUTED ASSUMPTIONS');
  const inputs = sim.result?.inputs;
  if (!Array.isArray(inputs) || !inputs.length) add('Saved input plans are unavailable.');
  for (const [planIndex, plan] of (Array.isArray(inputs) ? inputs : []).entries()) {
    add(
      `${plan.name || reports[planIndex]?.name || `Plan ${planIndex + 1}`} · ${plan.days ?? 'Unavailable'} days · ${money(plan, 'startingCash', plan.currency || reports[planIndex]?.currency)} opening cash`,
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
    }
  }
  add('Cash is shared across SKUs; product input order sets purchase priority.');
  add('Conditional projections, not validated forecasts. Esc returns to the lab.');
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
  }
  add('Enter loads assumptions only.');
  add('Run and save are separate actions.');
  return lines;
}

export function importReviewLimit(preview, columns = 80, rows = 24) {
  const w = Math.max(1, Math.min(220, columns - 1));
  const size = Math.max(1, w - (w >= 110 ? 22 : 2));
  const visible = Math.max(1, Math.min(100, rows - 1) - 9);
  return Math.max(0, importReviewLines(preview, size).length - visible);
}

export function panelLimit(state, columns = 80, rows = 24) {
  const w = Math.max(1, Math.min(220, columns - 1));
  const size = Math.max(1, w - (w >= 110 ? 22 : 2));
  const available = Math.max(1, Math.min(100, rows - 1) - 8);
  return Math.max(0, resultLines(state, size).length - Math.max(1, available - 1));
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

function staticBody(state) {
  const body = [];
  const put = (text = '', tone = 'ink', bold = false) => body.push({ text, tone, bold });
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
      'w / l / h       Save scenario / library / run history',
      'x / i / c       Export result / import assumptions / cancel run',
      '/ in library    Filter saved entries without reading the store',
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
export function contentLimit(state, _columns = 80, rows = 24) {
  const available = Math.max(1, Math.min(100, rows - 1) - 8);
  return Math.max(0, staticBody(state).length - available);
}

export function render(state, columns = 80, rows = 24, { color = true } = {}) {
  const w = Math.max(1, Math.min(220, columns - 1));
  const h = Math.max(1, Math.min(100, rows - 1));
  const style = (value, tone = 'ink', bold = false) =>
    color ? `\x1b[${bold ? '1;' : ''}38;5;${COLORS[tone]}m${value}\x1b[0m` : value;
  const out = [];
  const add = (value = '', tone = 'ink', bold = false) =>
    out.push(style(fit(value, w), tone, bold));
  add(' STATESET  /  iCommerce', 'accent', true);
  if (w < 38 || h < 12) {
    add(' Enlarge pane to at least 40 × 14.', 'warn');
    add(' q Quit');
    return out.slice(0, h).join('\n');
  }
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
  const body = [];
  const put = (text = '', tone = 'ink', bold = false) => body.push({ text, tone, bold });
  const sidebar = w >= 110;
  const size = w - (sidebar ? 22 : 2);
  const available = Math.max(1, h - 8);
  if (state.input?.kind === 'import-review') {
    const lines = importReviewLines(state.input.preview, size),
      visible = Math.max(1, available - 1);
    const offset = Math.min(state.input.offset, Math.max(0, lines.length - visible));
    put(
      `IMPORT PREVIEW · ${offset + 1}–${Math.min(lines.length, offset + visible)}/${lines.length} · ↑ ↓ scroll`,
      'accent',
      true,
    );
    lines.slice(offset, offset + visible).forEach((line) => put(line));
  } else if (state.input?.kind === 'planning' || state.input?.kind === 'product') {
    const editor = state.input;
    const fields =
      editor.kind === 'product'
        ? PRODUCT_FIELDS
        : state.products.length
          ? PLANNING_FIELDS.slice(0, 2)
          : PLANNING_FIELDS;
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
  } else if (state.panel === 'results' && !state.detail && !state.help && !state.input) {
    const lines = resultLines(state, size),
      visible = Math.max(1, available - 1);
    const offset = Math.min(state.panelOffset, Math.max(0, lines.length - visible));
    put(
      `COMPARISON · ${offset + 1}–${Math.min(lines.length, offset + visible)} / ${lines.length} lines · Esc lab`,
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
        scenarios: 'SAVED SCENARIOS · Enter load · / filter',
        history: 'RUN HISTORY · Enter inspect · a use · / filter',
        products: 'SELECTED SKUS · PURCHASE PRIORITY',
      }[state.panel],
      'accent',
      true,
    );
    const allRows = {
      scenarios: state.library.scenarios,
      history: state.library.runs,
      products: state.products,
    }[state.panel];
    if (state.panel === 'products') put('[ earlier · ] later · Enter edit · / filter', 'muted');
    put(
      `${rows.length} / ${allRows.length} entries${state.panelQuery ? ` · / ${state.panelQuery}` : ''} · ${state.panel === 'products' ? 'd' : 'Shift+D'} remove${state.undoRemoval ? ' · Shift+U undo' : ''}`,
      'muted',
    );
    const count = Math.max(1, Math.floor((available - (state.panel === 'products' ? 3 : 2)) / 2));
    const start = Math.max(0, state.selected - count + 1);
    rows.slice(start, start + count).forEach((item, i) => {
      put(
        `${start + i === state.selected ? '›' : ' '} ${state.panel === 'products' ? `${allRows.indexOf(item) + 1}. ` : ''}${item.name || item.sku || item.preset}`,
        start + i === state.selected ? 'accent' : 'ink',
      );
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
  } else if (['save', 'export', 'import', 'seed'].includes(state.input?.kind)) {
    put(
      state.input.kind === 'seed'
        ? 'SET SIMULATION SEED · 1–999999'
        : state.input.kind === 'save'
          ? 'SAVE SCENARIO · enter a name'
          : state.input.kind === 'import'
            ? 'IMPORT SCENARIO · enter a local JSON filename'
            : 'EXPORT RUN · enter a new JSON filename',
      'accent',
      true,
    );
    put('Enter confirm · Esc cancel', 'muted');
    put(
      state.input.kind === 'seed'
        ? 'Enter saves a valid seed. Existing results keep their original seed.'
        : state.input.kind === 'save'
          ? `Saves ${state.selectedPreset} with current seed, assumptions and SKUs.`
          : state.input.kind === 'import'
            ? 'Validate and preview exported assumptions. Nothing runs automatically.'
            : 'Includes executed inputs, input digest and the complete engine report.',
    );
    put(
      state.input.kind === 'seed'
        ? 'Use s to run the selected scenario.'
        : 'Existing files and saved entries are preserved.',
      'muted',
    );
  } else if (state.input?.kind === 'palette') {
    put('COMMAND PALETTE', 'accent', true);
    put('↑ ↓ select  ·  Enter open  ·  Type to filter  ·  Esc dismiss', 'muted');
    const commands = paletteCommands(state.input.text, state),
      selected = state.input.selected || 0;
    const start = Math.max(0, selected - Math.max(1, available - 2) + 1);
    commands
      .slice(start, start + Math.max(1, available - 2))
      .forEach((c, i) =>
        put(
          `${i + start === selected ? '› ' : '  '}${c.label}`,
          i + start === selected ? 'accent' : 'ink',
        ),
      );
    if (!commands.length) put('No matching commands.', 'muted');
  } else if (state.help) {
    body.push(...staticBody(state));
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
    body.push(...staticBody(state));
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
      state.view === 6
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
    body.push(...explorerBody(state, size, available));
  } else {
    const data = records(state),
      field = pageKey(state.view);
    if (state.view === 5) {
      put(`ISOLATED ENGINE  ·  Seed ${state.seed}  ·  g set · + / -`, 'good');
      put(
        `${state.planning.days} days · USD ${state.planning.startingCash} cash · e edit assumptions`,
        'muted',
      );
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
    } else
      put(
        `Page ${(state.data?.pages?.[field] || 0) + 1} · ${data.length} shown / ${state.data?.matched?.[field] ?? state.data?.counts?.[field] ?? '—'} matches · n / p · o sort`,
        'muted',
      );
    if (state.relation && state.relation.field === field)
      put(`Related ${state.relation.column}: ${state.relation.id} · b back`, 'muted');
    if (state.view !== 5)
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
    const listHeight =
      state.view === 5 ? Math.min(4, Math.max(1, available - 10)) : Math.max(1, available - 3);
    const start = Math.max(0, state.selected - listHeight + 1);
    for (const [i, record] of data.slice(start, start + listHeight).entries()) {
      const selected = i + start === state.selected;
      put(
        `${selected ? '›' : ' '} ${tableRow(state.view, record, size - 2)}`,
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
    if (state.view === 5) {
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
          JSON.stringify(sim.controls) !== JSON.stringify(state.planning) ||
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
      state.help || state.view === 0
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
    ` ${state.input?.error ? state.input.error : state.error ? `STALE / ERROR  ${state.error}` : errors.length ? `PARTIAL DATA  ${errors.map((e) => e.section).join(', ')}` : state.sim.running ? `RUNNING ${Math.floor((Date.now() - state.sim.startedAt) / 1000)}s · ${state.sim.progress || 'Engine working'} · c cancel` : state.notice || (state.sim.error ? `Run failed: ${state.sim.error}` : state.data ? `Snapshot ${state.data.checkedAt}  ·  r refresh` : 'Loading engine…')}`,
    state.error || errors.length || state.input?.error || state.sim.error ? 'warn' : 'muted',
  );
  add(
    state.input?.kind === 'import-review'
      ? ' Enter load assumptions  ↑ ↓ scroll  Esc cancel'
      : ['planning', 'product'].includes(state.input?.kind)
        ? ' Ctrl+U clear  Tab next  Ctrl+S save  Esc cancel'
        : state.input
          ? ` ${{ search: '/', palette: ':', save: 'Name: ', export: 'Path: ', import: 'Path: ', seed: 'Seed:' }[state.input.kind] || ''} ${editorWindow(state.input.text, state.input.cursor, Math.max(1, w - 36))}  Enter accept · Esc cancel`
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
