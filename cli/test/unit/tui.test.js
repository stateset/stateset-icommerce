import assert from 'node:assert/strict';
import { test } from 'node:test';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { clean, fit, width } from '../../src/tui/text.js';
import {
  initialState,
  handleKey,
  navigate,
  records,
  simulationPlans,
} from '../../src/tui/model.js';
import { render, detailLimit } from '../../src/tui/render.js';
import { startTerminal } from '../../src/tui/app.js';
import { readStore } from '../../src/tui/worker.js';
import { DEFAULT_PLANNING, planningControls } from '../../src/tui/planning.js';
import { cashDelta, fillRate, soldTraces } from '../../src/tui/report.js';
import { inventoryPlanComparisonSchema } from '../../src/simulation/schema.js';

test('untrusted terminal values cannot emit commands, clipboard OSC or bidi overrides', () => {
  const attack = '\x1b]52;c;ZXZpbA==\x07\x1b[2Jhi\r\n\u202ehidden';
  const text = clean(attack);
  assert.doesNotMatch(text, /[\x00-\x1f\x7f-\x9f\u202e]/);
  assert.match(text, /hi/);
  assert.equal(width('珈琲'), 4);
  assert.equal(width('e\u0301'), 1);
  assert.equal(width('📦'), 2);
  assert.equal(fit('珈琲', 3), '珈 ');
});

test('search owns all keystrokes; q and digits cannot dispatch commands while typing', () => {
  let state = navigate(initialState(), 1);
  state = handleKey(state, '/', { name: '/' }).state;
  for (const c of 'q6') {
    const response = handleKey(state, c, { name: c });
    assert.equal(response.effect, undefined);
    state = response.state;
  }
  state = handleKey(state, '', { name: 'return' }).state;
  assert.equal(state.query, 'q6');
  assert.equal(state.view, 1);
  assert.equal(handleKey(state, '', { ctrl: true, name: 'c' }).effect, 'quit');
});

test('navigation, filtering, pagination and detail movement remain bounded', () => {
  let state = navigate(initialState(), 1);
  state.data = {
    orders: [{ orderNumber: 'ORD-1' }, { orderNumber: 'ORD-2' }],
    counts: { orders: 51 },
  };
  for (let i = 0; i < 10; i++) state = handleKey(state, 'j', { name: 'j' }).state;
  assert.equal(state.selected, 1);
  const next = handleKey(state, 'n', { name: 'n' });
  assert.equal(next.effect, 'refresh');
  assert.equal(next.state.pages.orders, 1);
  assert.equal(handleKey(next.state, 'n', { name: 'n' }).effect, undefined);
  state.query = 'ORD-1';
  assert.equal(records(state).length, 1);
  state = handleKey(state, '', { name: 'tab' }).state;
  assert.equal(state.query, '');
  assert.equal(state.selected, 0);
});

test('palette exposes only fixed actions and never interprets shell input', () => {
  let state = handleKey(initialState(), '', { name: 'p', ctrl: true }).state;
  state.input.text = 'Orders';
  state = handleKey(state, '', { name: 'return' }).state;
  assert.equal(state.view, 1);
  state.input = { kind: 'palette', text: '$(touch /tmp/unsafe)' };
  assert.equal(handleKey(state, '', { name: 'return' }).effect, undefined);
});

test('palette arrow navigation selects a command and filtering resets selection', () => {
  let state = handleKey(initialState(), ':', {}).state;
  state = handleKey(state, '', { name: 'down' }).state;
  state = handleKey(state, '', { name: 'down' }).state;
  assert.equal(handleKey(state, '', { name: 'return' }).state.view, 2);
  state = handleKey(state, 'O', { name: 'o' }).state;
  assert.equal(state.input.selected, 0);
  state.input.text = 'Orders';
  assert.equal(handleKey(state, '', { name: 'return' }).state.view, 1);
});

test('search previews records while typing and Escape restores the committed filter', () => {
  let state = navigate(initialState(), 1);
  state.data = { orders: [{ orderNumber: 'ORD-A' }, { orderNumber: 'ORD-B' }] };
  state = handleKey(state, '/', {}).state;
  state = handleKey(state, 'ORD-A', {}).state;
  assert.equal(records(state).length, 1);
  assert.equal(state.query, '');
  state = handleKey(state, '', { name: 'escape' }).state;
  assert.equal(records(state).length, 2);
});

test('planning editor validates atomically, preserves canceled settings and blocks writes', () => {
  let state = handleKey(navigate(initialState(), 5), 'e', {}).state;
  state.input.draft.startingCash = '1e4';
  state = handleKey(state, '', { name: 's', ctrl: true }).state;
  assert.match(state.input.error, /decimal string/);
  assert.deepEqual(state.planning, DEFAULT_PLANNING);
  state.input.draft.startingCash = '0.01';
  state.input.draft.days = '3';
  state = handleKey(state, '', { name: 's', ctrl: true }).state;
  assert.equal(state.input, null);
  assert.equal(state.planning.startingCash, '0.01');
  assert.equal(state.planning.days, 3);
  state = handleKey(state, 'e', {}).state;
  state.input.draft.days = '9';
  state = handleKey(state, '', { name: 'escape' }).state;
  assert.equal(state.planning.days, 3);
  assert.throws(() => planningControls({ startingCash: 0.01 }), /decimal string/);
  assert.throws(() => planningControls({ days: 31 }), /horizon/);
  assert.throws(() => planningControls({ reorderPoint: 24, targetStock: 24 }), /above/);
  assert.throws(() => planningControls({ apply: true }), /Unknown/);
});

test('edited planning inputs produce bounded native scenarios and relative comparisons', () => {
  const controls = { ...DEFAULT_PLANNING, days: 30, startingCash: '0.01', leadTimeDays: 14 };
  const plans = simulationPlans('delay', 9, controls);
  inventoryPlanComparisonSchema.parse(plans);
  assert.equal(plans[0].days, 30);
  assert.equal(plans[1].products[0].leadTimeDays, 17);
  assert.equal(plans[0].startingCash, '0.01');
  assert.equal(simulationPlans('cash', 9, controls)[1].startingCash, '0.0005');
  assert.deepEqual(DEFAULT_PLANNING, initialState().planning);
});

test('financial comparisons remain exact and unit charts share a common scale', () => {
  const baseline = { currency: 'USD', summary: { endingCash: '9007199254740993.01' } };
  assert.equal(
    cashDelta(
      { currency: 'USD', summary: { endingCash: '9007199254740993.02' } },
      baseline,
      'endingCash',
    ),
    '+0.01',
  );
  assert.equal(
    cashDelta({ currency: 'EUR', summary: { endingCash: '1.00' } }, baseline, 'endingCash'),
    'n/a',
  );
  assert.equal(fillRate({ sold: 1, demanded: 3 }), '33.33%');
  assert.equal(fillRate({ sold: 0, demanded: 0 }), 'n/a');
  const trace = soldTraces(
    [5, 10].map((sold) => ({ days: 1, timeline: [{ type: 'demand', day: 1, sold }] })),
  );
  assert.equal(trace.peak, 10);
  assert.equal(trace.lines[0], '▄');
  assert.equal(trace.lines[1], '█');
});

test('editor and palette keep their active fields visible in small panes', () => {
  let state = handleKey(navigate(initialState(), 5), 'e', {}).state;
  state.input.field = 5;
  let screen = render(state, 55, 14, { color: false });
  assert.match(screen, /Supplier lead time/);
  state.input = { kind: 'palette', text: '', selected: 8 };
  screen = render(state, 55, 14, { color: false });
  assert.match(screen, /› Quit/);
  for (const line of screen.split('\n')) assert.ok(width(line) < 55);
});

test('long reports support bounded Home/End navigation and preserve structured fields', () => {
  const state = navigate(initialState(), 5);
  state.detail = true;
  state.sim.result = {
    inputs: Array.from({ length: 12 }, (_, i) => ({
      name: `Plan ${i}`,
      startingCash: '9007199254740993.01',
    })),
    lastField: 'report-end',
  };
  const limit = detailLimit(state, 55, 20);
  assert.ok(limit > 0);
  assert.equal(handleKey(state, '', { name: 'end' }).effect, 'detail-end');
  state.detailOffset = limit;
  assert.match(render(state, 55, 20, { color: false }), /report-end/);
  assert.equal(handleKey(state, '', { name: 'home' }).state.detailOffset, 0);
});

test('all scenario inputs use exact money, bounded seeds and independent baseline terms', () => {
  const [base, spike] = simulationPlans('spike', 42);
  assert.equal(base.products[0].demand.max, 5);
  assert.equal(spike.products[0].demand.max, 15);
  spike.products[0].unitPrice = '0.01';
  assert.equal(base.products[0].unitPrice, '12.50');
  assert.equal(simulationPlans('cash', 42)[1].startingCash, '10.00');
  assert.throws(() => simulationPlans('live-payment', 42));
  assert.throws(() => simulationPlans('spike', Infinity));
});

test('Enter while inspecting a simulation report returns without silently rerunning it', () => {
  const state = navigate(initialState(), 5);
  state.sim.result = { results: [] };
  state.detail = true;
  const next = handleKey(state, '', { name: 'return' });
  assert.equal(next.effect, undefined);
  assert.equal(next.state.detail, false);
});

test('every view fits narrow, wide and tiny panes with long hostile Unicode data', () => {
  const data = {
    counts: { orders: 1, customers: 1, products: 1, payments: 1 },
    orders: [
      {
        orderNumber: '珈琲'.repeat(100) + '\x1b]52;c;bad\x07',
        status: 'pending',
        totalAmount: 0.1,
        totalAmountExact: '9007199254740993.01',
        currency: 'USD',
      },
    ],
    inventory: [],
    customers: [],
    payments: [],
    errors: [],
  };
  for (const [columns, rows] of [
    [1, 1],
    [35, 10],
    [77, 44],
    [120, 30],
    [180, 50],
  ]) {
    for (let view = 0; view < 6; view++) {
      const output = render({ ...initialState(), view, data }, columns, rows, { color: false });
      assert.ok(output.split('\n').length <= Math.max(1, rows - 1));
      for (const line of output.split('\n')) assert.ok(width(line) <= Math.max(1, columns - 1));
      assert.doesNotMatch(output, /\x1b/);
    }
  }
  const detail = render({ ...initialState(), view: 1, data, detail: true }, 100, 40, {
    color: false,
  });
  assert.match(detail, /9007199254740993\.01/);
  assert.doesNotMatch(detail, /totalAmount: 0.1/);
});

test('a failed signal stays unavailable rather than becoming a false zero', async () => {
  const commerce = {
    orders: {
      count: async () => {
        throw new Error('Unavailable');
      },
      list: async () => [],
    },
    customers: { count: async () => 0, list: async () => [] },
    products: { count: async () => 0 },
    payments: { count: async () => 0, list: async () => [] },
    analytics: { inventoryHealth: async () => ({}), lowStockItems: async () => [] },
  };
  const data = await readStore(commerce);
  assert.equal(data.counts.orders, null);
  assert.equal(data.counts.customers, 0);
  assert.equal(data.errors[0].section, 'orderCount');
  assert.match(render({ ...initialState(), data }, 100, 30, { color: false }), /PARTIAL DATA/);
});

test('terminal exit restores raw mode, cursor and alternate screen and closes backend once', async () => {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter();
  let raw = false,
    closes = 0,
    frames = '';
  input.setRawMode = (value) => {
    raw = value;
  };
  output.columns = 77;
  output.rows = 44;
  output.on('data', (chunk) => {
    frames += chunk;
  });
  const backend = {
    request: async () => ({ counts: {}, errors: [], inventory: [] }),
    close: async () => {
      closes++;
    },
  };
  const done = startTerminal({ input, output, signals, backend, color: false });
  assert.equal(raw, true);
  input.emit('keypress', 'q', { name: 'q' });
  await done;
  assert.equal(raw, false);
  assert.equal(closes, 1);
  assert.match(frames, /\x1b\[\?1049h/);
  assert.match(frames, /\x1b\[\?25h\x1b\[\?1049l/);
  assert.equal(signals.listenerCount('SIGTERM'), 0);
});

test('failed runs preserve the successful report and replay its original inputs', async () => {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter();
  input.setRawMode = () => {};
  output.columns = 100;
  output.rows = 44;
  let frames = '',
    runs = 0;
  const calls = [];
  output.on('data', (chunk) => {
    frames += chunk;
  });
  const result = {
    results: [
      {
        name: 'Baseline',
        currency: 'USD',
        days: 7,
        summary: {
          sold: 1,
          demanded: 1,
          lostSales: 0,
          endingCash: '200.00',
          purchasingSpend: '0.00',
          grossMargin: '0.00',
          purchaseOrders: 0,
        },
        timeline: [],
      },
    ],
  };
  const backend = {
    close: async () => {},
    request: async (operation, payload) => {
      if (operation === 'snapshot') return { counts: {}, errors: [], inventory: [] };
      calls.push(structuredClone(payload));
      if (++runs === 2) throw new Error('Fixture run failed');
      return result;
    },
  };
  const flush = () => new Promise((resolve) => setImmediate(resolve));
  const done = startTerminal({ input, output, signals, backend, color: false });
  await flush();
  input.emit('keypress', '6', { name: '6' });
  input.emit('keypress', 's', { name: 's' });
  await flush();
  input.emit('keypress', '+', { name: '+' });
  input.emit('keypress', 's', { name: 's' });
  await flush();
  assert.match(frames, /Previous result retained/);
  assert.equal(calls[1].seed, 43);
  input.emit('keypress', 'R', { name: 'r', shift: true });
  await flush();
  assert.equal(calls[2].seed, 42);
  assert.deepEqual(calls[2].controls, calls[0].controls);
  input.emit('keypress', 'q', { name: 'q' });
  await done;
});
