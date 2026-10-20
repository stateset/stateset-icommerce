import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  initialState,
  navigate,
  handleKey,
  records,
  paletteCommands,
} from '../../src/tui/model.js';
import { terminalLayout } from '../../src/tui/layout.js';
import {
  render,
  detailLimit,
  contentLimit,
  panelLimit,
  scenarioUpdateLimit,
  scenarioUpdateLines,
  importReviewLimit,
  importReviewLines,
} from '../../src/tui/render.js';
import { width } from '../../src/tui/text.js';

const key = (state, text, name = text) => handleKey(state, text, { name });
const screen = (state, columns = 20, rows = 8) => render(state, columns, rows, { color: false });
const data = () => ({
  counts: { orders: 2, inventory: 2, customers: 2, products: 2, payments: 2 },
  errors: [],
  orders: [
    { id: 'o1', orderNumber: 'ORDER-1', status: 'pending' },
    { id: 'o2', orderNumber: 'ORDER-2', status: 'confirmed' },
  ],
  inventory: [
    { sku: 'STOCK-1', available: '0' },
    { sku: 'STOCK-2', available: '5' },
  ],
  customers: [
    { id: 'c1', email: 'a@store.test' },
    { id: 'c2', email: 'b@store.test' },
  ],
  payments: [
    { id: 'p1', paymentNumber: 'PAY-1', status: 'captured' },
    { id: 'p2', paymentNumber: 'PAY-2', status: 'refunded' },
  ],
});
const state = (view) => ({ ...navigate(initialState('DEMO'), view), data: data() });

test('compact lists keep the selected item visible across every view at 20 by 8', () => {
  const labels = {
    1: 'ORDER-2',
    2: 'STOCK-2',
    3: 'b@store.test',
    4: 'PAY-2',
    5: 'Daily reviews',
  };
  for (let view = 1; view <= 5; view++) {
    const selected = key(state(view), '', 'end').state;
    assert.ok(screen(selected).includes(labels[view]), `${view}: ${screen(selected)}`);
    assert.match(screen(selected), /›/);
    assert.doesNotMatch(screen(selected), /Enlarge|Need 20/);
  }
  assert.match(screen(state(0)), /orders: 2/);
  const domains = key(state(6), '', 'end').state;
  assert.match(screen(domains), /DOMAINS/);
  assert.ok(screen(domains).includes(records(domains)[domains.selected].name.slice(0, 10)));
  const help = {
    ...state(0),
    help: true,
    contentOffset: contentLimit({ ...state(0), help: true }, 20, 8),
  };
  assert.match(screen(help), /forecasts\./);
});

test('compact editors and text prompts show the active field and cursor with input ownership intact', () => {
  let editor = key(state(5), 'e').state;
  editor = key(editor, '', 'tab').state;
  assert.match(screen(editor), /Opening cash/);
  editor = handleKey(editor, '', { name: 'u', ctrl: true }).state;
  editor = key(editor, '999999.99').state;
  assert.match(screen(editor), /999999.99▏/);
  const saved = handleKey(editor, '', { name: 's', ctrl: true }).state;
  assert.equal(saved.planning.startingCash, '999999.99');
  assert.equal(saved.input, null);
  const prompt = key(key(saved, 'w').state, 'Safe q7 name').state;
  assert.match(screen(prompt), /Safe q7 name▏/);
  assert.match(screen(prompt), /Enter \/ Esc cancel/);
  assert.equal(prompt.view, 5);
  const palette = {
    ...state(0),
    contentOffset: 999,
    input: {
      kind: 'palette',
      text: '',
      selected: paletteCommands('', state(0)).findIndex((command) => command.effect === 'quit'),
    },
  };
  assert.match(screen(palette), /› Quit/);
});

test('compact library selection remains visible and a filtered update review can reach its final instructions', () => {
  const base = state(5);
  base.library.scenarios = Array.from({ length: 20 }, (_, index) => ({
    id: `s${index}`,
    name: `Plan ${index}`,
    presetId: 'baseline',
    seed: 42,
    controls: { ...base.planning },
    products: [],
  }));
  const library = key(key(base, 'l').state, '', 'end').state;
  assert.match(screen(library), /› Plan 19/);
  assert.match(screen(library), /20\/20/);
  const reviewed = key(library, 'S').state;
  const limit = scenarioUpdateLimit(reviewed.input, 20, 8);
  const end = { ...reviewed, input: { ...reviewed.input, offset: limit } };
  const { size, available } = terminalLayout(20, 8);
  const lines = scenarioUpdateLines(reviewed.input, size);
  assert.equal(limit, lines.length - (available - 1));
  for (const line of lines.slice(limit)) assert.ok(screen(end).includes(line));
  assert.match(screen(end), /Enter replace Esc/);
  assert.equal(key(reviewed, 's').effect, undefined);
});

test('exact report values remain fully inspectable through compact detail scrolling', () => {
  const detail = {
    ...state(1),
    detail: true,
    data: { ...data(), orders: [{ amountExact: '9007199254740993.01', finalField: 'report-end' }] },
  };
  const limit = detailLimit(detail, 20, 8),
    frames = [];
  for (let offset = 0; offset <= limit; offset++)
    frames.push(screen({ ...detail, detailOffset: offset }));
  assert.ok(
    frames.some(
      (frame) =>
        frame
          .split('\n')
          .slice(3, 5)
          .map((line) => line.trim())
          .join('') === '9007199254740993.01',
    ),
  );
  assert.ok(frames.some((frame) => frame.includes('report-end')));
  assert.ok(limit > detailLimit(detail, 140, 44));
  const end = screen({ ...detail, detailOffset: limit });
  assert.match(end, /report-end/);
});

test('compact review and result limits use their actual viewport on both sides of a resize', () => {
  const base = state(5),
    preview = {
      filename: 'run.json',
      cliVersion: '1.39.0',
      settings: { presetId: 'baseline', seed: 42, controls: base.planning, products: [] },
    };
  for (const [columns, rows] of [
    [20, 8],
    [35, 10],
    [39, 44],
    [140, 10],
    [40, 14],
    [140, 44],
  ]) {
    const { size, available } = terminalLayout(columns, rows);
    const visible = Math.max(1, available - 1);
    assert.equal(
      importReviewLimit(preview, columns, rows),
      Math.max(0, importReviewLines(preview, size).length - visible),
    );
    const report = {
      ...base,
      sim: { ...base.sim, preset: 'Baseline', seed: 42, result: { results: [], inputs: [] } },
      panel: 'results',
    };
    const end = screen(
      { ...report, panelOffset: panelLimit(report, columns, rows) },
      columns,
      rows,
    );
    assert.match(end.replace(/\s+/g, ' '), /lab\./);
  }
});

test('compact layouts fit every physical viewport and sanitize hostile text while retaining warnings', () => {
  for (const [columns, rows] of [
    [1, 1],
    [19, 8],
    [20, 7],
    [20, 8],
    [21, 44],
    [35, 10],
    [120, 10],
    [40, 14],
    [140, 44],
  ]) {
    for (let view = 0; view < 7; view++) {
      const s = state(view);
      s.source = 'DEMO\x1b]52;c;bad\x07\u202e';
      const output = screen(s, columns, rows);
      assert.ok(output.split('\n').length <= Math.max(1, rows - 1));
      for (const line of output.split('\n')) assert.ok(width(line) <= Math.max(1, columns - 1));
      assert.doesNotMatch(output, /[\x1b\u202e]/);
    }
  }
  assert.match(
    screen({ ...state(0), data: { ...data(), errors: [{ section: 'orders' }] } }),
    /PARTIAL DATA/,
  );
  assert.match(screen({ ...state(5), sim: { running: true, startedAt: Date.now() } }), /c cancel/);
  assert.match(screen(state(0), 19, 8), /Need 20 × 8/);
});
