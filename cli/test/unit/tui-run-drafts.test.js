import assert from 'node:assert/strict';
import { test } from 'node:test';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import {
  initialState,
  navigate,
  handleKey,
  paletteCommands,
  simulationPlans,
  PRESETS,
} from '../../src/tui/model.js';
import { runRecord, useRunAssumptions } from '../../src/tui/workspace.js';
import { productEditor, productFromDraft } from '../../src/tui/basket.js';
import { startTerminal } from '../../src/tui/app.js';
import { render } from '../../src/tui/render.js';

const tick = () => new Promise((resolve) => setImmediate(resolve));
function successful(presetId = 'cash', seed = 77, startingCash = '10.00') {
  const controls = { ...initialState().planning, days: 2, startingCash };
  const products = ['SECOND', 'FIRST'].map((sku) =>
    productFromDraft(sku, {
      ...productEditor({ sku, available: '7', priceExact: '2.00' }).draft,
      unitCost: '1.00',
    }),
  );
  return runRecord({
    presetId,
    preset: PRESETS.find((p) => p.id === presetId).name,
    seed,
    controls,
    products,
    result: {
      success: true,
      inputs: simulationPlans(presetId, seed, controls, products),
      results: [],
    },
  });
}

test('loading a run creates an independent editable draft and preserves original reports and priority', () => {
  const run = successful();
  const original = structuredClone(run);
  const state = {
    ...navigate(initialState(), 5),
    panel: 'results',
    detail: true,
    sim: { ...initialState().sim, result: { prior: true } },
  };
  const next = useRunAssumptions(state, run);
  assert.equal(next.panel, null);
  assert.equal(next.detail, false);
  assert.equal(next.seed, 77);
  assert.equal(next.selectedPreset, 'cash');
  assert.equal(next.selected, 3);
  assert.equal(
    next.planning.startingCash,
    '10.00',
    'restore baseline controls, not the 0.50 candidate',
  );
  assert.deepEqual(
    next.products.map((p) => p.sku),
    ['SECOND', 'FIRST'],
  );
  assert.equal(next.sim, state.sim, 'existing result is retained');
  assert.deepEqual(
    simulationPlans(next.selectedPreset, next.seed, next.planning, next.products),
    run.result.inputs,
  );
  next.planning.startingCash = '20.00';
  next.products[0].unitCost = '2.00';
  assert.deepEqual(run, original, 'editing the draft cannot mutate the run');
});

test('changing only the comparison preset marks the retained report as using older assumptions', () => {
  const run = successful('cash');
  const state = {
    ...navigate(initialState(), 5),
    selectedPreset: 'cash',
    selected: 3,
    seed: run.seed,
    planning: run.controls,
    products: run.products,
    sim: { ...run, running: false },
  };
  assert.doesNotMatch(
    render(state, 140, 44, { color: false }),
    /Assumptions changed since this result/,
  );
  const next = useRunAssumptions(state, successful('baseline'));
  assert.deepEqual(next.planning, state.planning);
  assert.deepEqual(next.products, state.products);
  assert.equal(next.seed, state.seed);
  assert.equal(next.sim, state.sim);
  assert.match(render(next, 140, 44, { color: false }), /Assumptions changed since this result/);
});

test('invalid run inputs, mismatched digests and active simulations refuse draft loading atomically', () => {
  const state = navigate(initialState(), 5),
    before = structuredClone(state),
    run = successful();
  assert.throws(
    () => useRunAssumptions(state, { ...run, inputDigest: 'wrong' }),
    /digest mismatch/,
  );
  assert.throws(
    () => useRunAssumptions(state, { ...run, seed: 78 }),
    /differ from executed inputs/,
  );
  assert.throws(
    () => useRunAssumptions(state, { ...run, result: { ...run.result, success: false } }),
    /differ from executed inputs/,
  );
  assert.throws(
    () => useRunAssumptions({ ...state, sim: { running: true } }, run),
    /active simulation/,
  );
  assert.deepEqual(state, before);
});

test('draft action is contextual, respects owned input and never dispatches simulation', () => {
  const state = {
    ...navigate(initialState(), 5),
    panel: 'history',
    library: { scenarios: [], runs: [successful()] },
  };
  assert.equal(handleKey(state, 'a', {}).effect, 'use-run-assumptions');
  assert.ok(paletteCommands('editable draft', state).some((command) => command.key === 'a'));
  for (const blocked of [
    { ...state, sim: { running: true } },
    { ...state, panelQuery: 'no-such-run' },
    { ...state, panel: null },
  ]) {
    assert.equal(handleKey(blocked, 'a', {}).effect, undefined);
    assert.equal(paletteCommands('editable draft', blocked).length, 0);
  }
  const editing = { ...state, input: { kind: 'search', text: '', cursor: 0 } };
  const typed = handleKey(editing, 'a', {});
  assert.equal(typed.effect, undefined);
  assert.equal(typed.state.input.text, 'a');
  const result = { ...state, panel: 'results', sim: { result: successful().result } };
  assert.equal(handleKey(result, 'a', {}).effect, 'use-run-assumptions');
});

test('filtered history loads the selected run through the app without executing or saving until requested', async (t) => {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter();
  input.setRawMode = () => {};
  output.columns = 120;
  output.rows = 44;
  let text = '';
  output.on('data', (chunk) => {
    text += chunk.toString();
  });
  const runs = [successful('baseline', 31, '11.00'), successful('delay', 43, '37.00')];
  const original = structuredClone(runs),
    requests = [],
    saves = [];
  const library = { scenarios: [], runs };
  const workspace = {
    load: async () => library,
    saveScenario: async (name, settings) => {
      saves.push({ name, settings });
      return library;
    },
  };
  const backend = {
    close: async () => {},
    request: async (operation, payload) => {
      requests.push({ operation, payload });
      if (operation !== 'snapshot') throw new Error('Loading assumptions must not execute');
      return { counts: {}, errors: [], orders: [], customers: [], inventory: [], payments: [] };
    },
  };
  const done = startTerminal({ input, output, signals, backend, workspace, color: false });
  t.after(async () => {
    input.emit('keypress', 'q', { name: 'q' });
    await done;
  });
  await tick();
  const send = (value, name = value) => input.emit('keypress', value, { name });
  send('6');
  send('h');
  send('/');
  send('Supplier delay');
  send('', 'return');
  send('a');
  assert.match(text, /Run assumptions loaded/);
  assert.deepEqual(
    requests.map((request) => request.operation),
    ['snapshot'],
  );
  assert.equal(saves.length, 0);
  send('w');
  send('Forked history');
  send('', 'return');
  await tick();
  assert.equal(saves.length, 1);
  assert.equal(saves[0].name, 'Forked history');
  assert.equal(saves[0].settings.presetId, 'delay');
  assert.equal(saves[0].settings.seed, 43);
  assert.equal(saves[0].settings.controls.startingCash, '37.00');
  assert.deepEqual(saves[0].settings.products, runs[1].products);
  assert.deepEqual(runs, original);
  assert.deepEqual(
    requests.map((request) => request.operation),
    ['snapshot'],
  );
});
