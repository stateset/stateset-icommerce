import { VIEWS, records, pageKey, paletteCommands } from './model.js';
import { clean, fit, width, wrap } from './text.js';
import { PLANNING_FIELDS } from './planning.js';
import { cashDelta, fillRate, soldTraces } from './report.js';

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
  const record = state.view === 5 ? state.sim.result : records(state)[state.selected];
  return Math.max(0, detailLines(record, size).length - visible);
}

function tableRow(view, row, size) {
  if (view === 1)
    return `${fit(shortId(row.orderNumber || row.id), 15)} ${fit(row.status, 13)} ${money(row, 'totalAmountExact')}`;
  if (view === 2)
    return `${fit(row.sku, Math.min(20, Math.floor(size / 3)))} ${fit(row.available, 10)} ${fit(row.reorderPoint ?? '—', 10)} ${row.name}`;
  if (view === 3)
    return `${fit(`${row.firstName} ${row.lastName}`, Math.min(24, Math.floor(size / 2)))} ${row.email}`;
  if (view === 4)
    return `${fit(shortId(row.paymentNumber || row.id), 15)} ${fit(row.status, 13)} ${money(row, 'amountExact')}`;
  return `${fit(row.name, 23)} ${row.note}`;
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
      VIEWS.map(
        (v, i) => `${i + 1} ${['Home', 'Orders', 'Stock', 'People', 'Pay', 'Lab'][i]}`,
      ).join('  '),
    'muted',
  );
  add(
    ` ${VIEWS[state.view]}${(state.input?.kind === 'search' ? state.input.text : state.query) ? `  / ${state.input?.kind === 'search' ? state.input.text : state.query}` : ''}${state.loading ? '  ·  Refreshing…' : ''}`,
    'accent',
    true,
  );
  const body = [];
  const put = (text = '', tone = 'ink', bold = false) => body.push({ text, tone, bold });
  const sidebar = w >= 110;
  const size = w - (sidebar ? 22 : 2);
  const available = Math.max(1, h - 8);
  if (state.input?.kind === 'planning') {
    const editor = state.input;
    put('PLANNING ASSUMPTIONS  ·  private scenarios only', 'accent', true);
    put('Tab / Enter next  ·  Ctrl+U clear  ·  Ctrl+S save  ·  Esc cancel', 'muted');
    const visible = Math.max(1, available - 4),
      start = Math.max(0, editor.field - visible + 1);
    for (const [index, field] of PLANNING_FIELDS.slice(start, start + visible).entries()) {
      const active = start + index === editor.field;
      put(
        `${active ? '›' : ' '} ${fit(field.label, Math.min(23, size - 16))} [${editor.draft[field.key]}]`,
        active ? 'accent' : 'ink',
        active,
      );
      if (active) put(`  ${field.hint}`, 'muted');
    }
    if (editor.error) put(editor.error, 'bad');
    else put('Changes affect the next run. Existing results keep their inputs.', 'muted');
  } else if (state.input?.kind === 'palette') {
    put('COMMAND PALETTE', 'accent', true);
    put('↑ ↓ select  ·  Enter open  ·  Type to filter  ·  Esc dismiss', 'muted');
    const commands = paletteCommands(state.input.text),
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
    put('YOUR COMMERCE WORKBENCH', 'accent', true);
    for (const line of [
      '1–6 / Tab       Switch view',
      '↑ ↓ / j k       Select a record or scenario',
      'Enter           Open details / run selected simulation',
      'Esc             Back / clear search',
      '/               Search this loaded page',
      ': / Ctrl+P      Command palette',
      'n / p           Next / previous data page',
      'r               Refresh the store snapshot',
      's               Run selected simulation in the lab',
      'e               Edit planning assumptions',
      'Shift+R         Replay last successful simulation inputs',
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
  } else if (!state.data && state.error && state.view !== 5) {
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
    const inventory = state.data?.inventory;
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
    put('6  Open the simulation lab', 'accent');
    put('Compare demand, supplier delays and cash constraints.');
    put('Every scenario runs in a separate embedded store.', 'muted');
    put();
    put('RECENT ORDERS', 'muted');
    for (const order of (state.data?.orders || []).slice(0, 3))
      put(
        `${shortId(order.orderNumber || order.id)}  ${order.status}  ${money(order, 'totalAmountExact')}`,
      );
  } else if (state.detail) {
    const lines = detailLines(
      state.view === 5 ? state.sim.result : records(state)[state.selected],
      size,
    );
    const offset = Math.min(state.detailOffset, Math.max(0, lines.length - available + 1));
    put('RECORD DETAILS  ·  ↑ ↓ scroll  ·  Esc back', 'muted');
    lines.slice(offset, offset + available - 1).forEach((line) => put(line));
  } else {
    const data = records(state),
      field = pageKey(state.view);
    if (state.view === 5) {
      put(`ISOLATED ENGINE  ·  Seed ${state.seed}  ·  + / - to change`, 'good');
      put(
        `${state.planning.days} days · USD ${state.planning.startingCash} cash · e edit assumptions`,
        'muted',
      );
      put(
        `Stock ${state.planning.initialStock} · Reorder ${state.planning.reorderPoint} → ${state.planning.targetStock} · Lead ${state.planning.leadTimeDays}d`,
        'muted',
      );
    } else if (state.view === 2) put('LOW STOCK  ·  Available ≤ 10  ·  First 50 signals', 'muted');
    else
      put(
        `Page ${(state.data?.pages?.[field] || 0) + 1}  ·  ${data.length} shown / ${state.data?.counts?.[field] ?? '—'} total  ·  n / p pages`,
        'muted',
      );
    if (state.view !== 5)
      put(
        [
          '',
          'ORDER           STATUS        TOTAL',
          'SKU                  AVAILABLE  REORDER    NAME',
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
        put(`RESULTS  ${sim.preset} / seed ${sim.seed}  ·  v report  ·  R replay`, 'good', true);
        const reports = sim.result.results || [],
          baseline = reports[0];
        for (const report of reports) {
          const summary = report.summary;
          put(
            `${report.name}  ·  fill ${fillRate(summary)}  ·  lost ${summary.lostSales}`,
            'accent',
          );
          put(
            `Cash ${money(summary, 'endingCash', report.currency)}  ·  buying ${money(summary, 'purchasingSpend', report.currency)}`,
          );
          put(
            `Margin ${money(summary, 'grossMargin', report.currency)}  ·  ${summary.purchaseOrders} purchase orders`,
            'muted',
          );
        }
        if (reports.length === 2) {
          put(
            `Change vs baseline: cash ${cashDelta(reports[1], baseline, 'endingCash')}`,
            'accent',
          );
          put(
            `                   margin ${cashDelta(reports[1], baseline, 'grossMargin')}`,
            'accent',
          );
        }
        const traces = soldTraces(reports, Math.min(24, Math.max(1, size - 28)));
        put(`SOLD UNITS · ${traces.bucket}d per cell · shared peak ${traces.peak}`, 'muted');
        reports.forEach((report, index) =>
          put(`${fit(report.name, 22)} ${traces.lines[index]}`, 'accent'),
        );
        if (
          sim.controls &&
          (JSON.stringify(sim.controls) !== JSON.stringify(state.planning) ||
            sim.seed !== state.seed)
        )
          put('Assumptions changed since this result. s runs the new settings.', 'warn');
      } else if (!sim.running && !sim.error)
        put('Enter / s runs a scenario; e edits its planning assumptions.', 'muted');
      put('Conditional projections from supplied assumptions.', 'muted');
    }
  }
  for (let i = 0; i < available; i++) {
    const item = body[i] || { text: '', tone: 'ink' };
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
    ` ${state.input?.kind === 'planning' && state.input.error ? state.input.error : state.error ? `STALE / ERROR  ${state.error}` : errors.length ? `PARTIAL DATA  ${errors.map((e) => e.section).join(', ')}` : state.notice || (state.data ? `Snapshot ${state.data.checkedAt}  ·  r refresh` : 'Loading engine…')}`,
    state.error || errors.length || state.input?.error ? 'warn' : 'muted',
  );
  add(
    state.input?.kind === 'planning'
      ? ' Ctrl+U clear  Tab next  Ctrl+S save  Esc cancel'
      : state.input
        ? ` ${state.input.kind === 'search' ? '/' : ':'} ${clean(state.input.text)}▏  Enter accept · Esc cancel`
        : ' / search   Enter inspect   : commands   ? help   q quit',
    state.input ? 'accent' : 'muted',
  );
  // Pad each row; stay off the last physical column/row to avoid wrapping and
  // terminal scrolling, including when Herdr resizes a split pane.
  return out
    .slice(0, h)
    .map((line) => line + ' '.repeat(Math.max(0, w - width(line))))
    .join('\n');
}
