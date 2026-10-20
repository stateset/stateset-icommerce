import assert from 'node:assert/strict';
import { test } from 'node:test';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { timelineEntries } from '../../src/tui/report.js';
import {
  initialState,
  navigate,
  handleKey,
  paletteCommands,
  simulationPlans,
} from '../../src/tui/model.js';
import { timelineLines, render, panelLimit } from '../../src/tui/render.js';
import { startTerminal } from '../../src/tui/app.js';
import { width } from '../../src/tui/text.js';

const events = () => [
  {
    day: 0,
    type: 'purchase_order_sent',
    sku: 'FIRST',
    quantity: 10,
    amount: '9007199254740993.01',
    cash: '0.0005',
    arrivalDay: 1,
  },
  {
    day: 0,
    type: 'purchase_budget_blocked',
    sku: 'SECOND',
    quantity: 10,
    required: '10.00',
    cash: '0.00',
  },
  {
    day: 1,
    type: 'purchase_received',
    sku: 'FIRST',
    quantity: 4,
    remaining: 6,
    complete: false,
    nextArrivalDay: 2,
  },
  {
    day: 1,
    type: 'demand',
    sku: 'FIRST',
    demand: 0,
    sold: 0,
    lost: 0,
    available: 4,
    cash: '0.0005',
  },
  {
    day: 1,
    type: 'purchase_budget_blocked',
    sku: 'SECOND',
    quantity: 10,
    required: '10.00',
    cash: '0.00',
  },
];
const report = () => ({
  name: 'Baseline',
  currency: 'USD',
  days: 2,
  summary: { sold: 0, demanded: 0, lostSales: 0 },
  inventory: [],
  timeline: events(),
});
const state = () => ({
  ...navigate(initialState('DEMO'), 5),
  panel: 'results',
  sim: {
    ...initialState().sim,
    preset: 'Steady demand',
    seed: 42,
    result: { success: true, results: [report()] },
  },
});
const key = (s, text, name = text) => handleKey(s, text, { name });
const tick = () => new Promise((resolve) => setImmediate(resolve));

test('timeline filters SKU and event words without sorting, merging repeated attempts or changing recorded indexes', () => {
  const source = report(),
    before = structuredClone(source);
  assert.deepEqual(
    timelineEntries(source, 'SECOND budget blocked').entries.map((item) => item.index),
    [1, 4],
  );
  assert.equal(timelineEntries(source, 'purchase_budget_blocked').entries.length, 2);
  assert.equal(timelineEntries(source, 'FIRST received').entries[0].index, 2);
  assert.equal(timelineEntries(source, '').entries[0].event.day, 0);
  assert.equal(timelineEntries(source, 'missing').total, 5);
  assert.deepEqual(timelineEntries(source, 'missing').entries, []);
  assert.deepEqual(source, before);
});

test('timeline renders exact recorded money, quantities, partial receipts and missing evidence without inferring currency', () => {
  const s = key(state(), 't').state;
  s.sim.result.results[0].timeline[0].constructor = 'recorded constructor metadata';
  let text = timelineLines(s, 160).join('\n');
  assert.match(text, /constructor: recorded constructor metadata/);
  assert.match(text, /1\. Day 0 · purchase order sent · FIRST/);
  assert.match(text, /Purchase spend: USD 9007199254740993\.01/);
  assert.match(text, /Cash after event: USD 0\.0005/);
  assert.match(text, /Remaining units: 6/);
  assert.match(text, /complete: No/);
  assert.match(text, /sold: 0/);
  assert.match(text, /Next arrival day: 2/);
  delete s.sim.result.results[0].currency;
  text = timelineLines(s, 160).join('\n');
  assert.match(text, /Purchase spend: Unavailable/);
  assert.doesNotMatch(text, /USD/);
  delete s.sim.result.results[0].timeline;
  assert.match(timelineLines(s, 160).join('\n'), /timeline unavailable/);
  s.sim.result.results[0].timeline = [];
  assert.match(timelineLines(s, 160).join('\n'), /No recorded events/);
  s.sim.result.results[0].timeline = [null, { day: 0, type: 'purchase_order_sent', amount: 0.1 }];
  s.sim.result.results[0].currency = 'USD';
  text = timelineLines(s, 160).join('\n');
  assert.match(text, /Invalid recorded event/);
  assert.match(text, /Purchase spend: Unavailable/);
});

test('timeline key and palette are contextual, filters own keys, and Escape returns through filter then results', () => {
  const original = state(),
    opened = key(original, 't');
  assert.equal(opened.effect, undefined);
  assert.equal(opened.state.panel, 'timeline');
  assert.ok(paletteCommands('event timeline', original).some((command) => command.key === 't'));
  for (const blocked of [
    { ...original, sim: { ...original.sim, running: true } },
    { ...original, panel: 'history' },
    { ...original, sim: { result: null } },
  ]) {
    assert.notEqual(key(blocked, 't').state.panel, 'timeline');
    assert.equal(paletteCommands('event timeline', blocked).length, 0);
  }
  let s = key(opened.state, '', 'end').state;
  assert.equal(s.panelOffset, Number.MAX_SAFE_INTEGER);
  s = key(key(s, '/').state, 'SECOND budget').state;
  assert.equal(s.panelOffset, 0);
  assert.equal(s.input.text, 'SECOND budget');
  assert.equal(s.sim, original.sim);
  s = key(s, '', 'return').state;
  assert.match(timelineLines(s, 160).join('\n'), /2\/5 recorded events/);
  assert.equal(key(s, 's').effect, undefined);
  assert.equal(key(s, 'a').effect, 'use-run-assumptions');
  const summary = key(s, 'f').state;
  assert.equal(summary.panel, 'results');
  assert.equal(summary.panelQuery, '');
  s = key(s, '', 'escape').state;
  assert.equal(s.panel, 'timeline');
  assert.equal(s.panelQuery, '');
  s = key(s, '', 'escape').state;
  assert.equal(s.panel, 'results');
  assert.equal(key(key(original, 't').state, 't').state.panel, 'results');
});

test('each plan keeps its own timeline and missing, empty and filtered-out timelines remain distinct', () => {
  const s = key(state(), 't').state;
  s.sim.result.results.push({ ...report(), name: 'Candidate', timeline: [events()[4]] });
  s.panelQuery = 'SECOND budget';
  const text = timelineLines(s, 160).join('\n');
  assert.match(text, /Baseline · 2\/5 recorded events/);
  assert.match(text, /Candidate · 1\/1 recorded events/);
  assert.match(text, /5\. Day 1 · purchase budget blocked/);
  assert.match(text, /1\. Day 1 · purchase budget blocked/);
  s.panelQuery = 'no-such-sku';
  assert.match(timelineLines(s, 160).join('\n'), /No events match/);
  assert.doesNotMatch(timelineLines(s, 160).join('\n'), /No recorded events/);
});

test('timeline scrolling reaches the final event across compact and wide resizes without terminal injection', () => {
  const s = key(state(), 't').state;
  s.sim.result.results[0].timeline[4].reason = '\x1b]52;c;bad\x07\u202eLAST-RECORDED-REASON';
  for (const [columns, rows] of [
    [20, 8],
    [40, 14],
    [140, 44],
  ]) {
    const frames = [];
    for (let panelOffset = 0; panelOffset <= panelLimit(s, columns, rows); panelOffset++) {
      const frame = render({ ...s, panelOffset }, columns, rows, { color: false });
      assert.ok(frame.split('\n').every((line) => width(line) <= columns - 1));
      assert.ok(frame.split('\n').length <= rows - 1);
      assert.doesNotMatch(frame, /[\x1b\u202e]/);
      frames.push(frame);
    }
    const text = frames.join('\n');
    assert.match(text, /LAST-RECORDED/);
    assert.match(
      render({ ...s, panelOffset: panelLimit(s, columns, rows) }, columns, rows, {
        color: false,
      }).replace(/\s+/g, ' '),
      /recorded event/,
    );
  }
});

test('inspecting and filtering stored history through the app dispatches no simulation and exports the complete original report', async (t) => {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter(),
    calls = [],
    exports = [];
  input.setRawMode = () => {};
  output.columns = 140;
  output.rows = 44;
  let frame = '';
  output.on('data', (chunk) => {
    frame = chunk.toString();
  });
  const controls = initialState().planning;
  const run = {
    id: 'run-1',
    presetId: 'baseline',
    preset: 'Steady demand',
    seed: 42,
    controls,
    products: [],
    result: {
      success: true,
      inputs: simulationPlans('baseline', 42, controls),
      results: [report()],
    },
  };
  const before = structuredClone(run);
  const workspace = {
    load: async () => ({ scenarios: [], runs: [run] }),
    exportRun: async (destination, record) => {
      exports.push({ destination, record });
      return destination;
    },
  };
  const backend = {
    close: async () => {},
    request: async (operation) => {
      calls.push(operation);
      if (operation !== 'snapshot') throw new Error('Read-only timeline must not execute');
      return { counts: {}, errors: [] };
    },
  };
  const done = startTerminal({ input, output, signals, backend, workspace, color: false });
  t.after(async () => {
    input.emit('keypress', '', { ctrl: true, name: 'c' });
    await done;
  });
  await tick();
  const send = (text, name = text) => input.emit('keypress', text, { name });
  send('6');
  send('h');
  send('', 'return');
  send('t');
  send('/');
  send('SECOND budget');
  send('', 'return');
  assert.match(frame, /2\/5 recorded events/);
  assert.doesNotMatch(frame, /purchase order sent/);
  output.columns = 20;
  output.rows = 8;
  output.emit('resize');
  send('', 'end');
  send('', 'up');
  assert.match(frame, /EVENTS/);
  output.columns = 140;
  output.rows = 44;
  output.emit('resize');
  send('', 'home');
  send('x');
  send('full-run.json');
  send('', 'return');
  await tick();
  assert.equal(exports.length, 1);
  assert.deepEqual(exports[0].record, before);
  assert.equal(exports[0].record.result.results[0].timeline.length, 5);
  assert.deepEqual(calls, ['snapshot']);
  assert.deepEqual(run, before);
});
