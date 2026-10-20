import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { TerminalWorkspace, runRecord } from '../../src/tui/workspace.js';
import {
  initialState,
  navigate,
  handleKey,
  paletteCommands,
  simulationPlans,
} from '../../src/tui/model.js';
import { productEditor, productFromDraft } from '../../src/tui/basket.js';
import { startTerminal } from '../../src/tui/app.js';
import { render, scenarioUpdateLines, scenarioUpdateLimit } from '../../src/tui/render.js';

const tick = () => new Promise((resolve) => setImmediate(resolve));
const settings = () => ({
  presetId: 'delay',
  seed: 77,
  controls: { ...initialState().planning, startingCash: '10.00' },
  products: ['SECOND', 'FIRST'].map((sku) =>
    productFromDraft(sku, {
      ...productEditor({ sku, available: '3', priceExact: '2.00' }).draft,
      unitCost: '1.00',
      minimumOrderQuantity: '4',
      orderMultiple: '2',
    }),
  ),
});
const library = () => ({
  scenarios: [
    { id: 'a', name: 'First', ...settings() },
    { id: 'b', name: 'Selected', ...settings() },
  ],
  runs: [],
});
const panel = () => ({
  ...navigate(initialState(), 5),
  panel: 'scenarios',
  panelQuery: 'Selected',
  library: library(),
});
const key = (state, text, name = text) => handleKey(state, text, { name });

test('confirmed updates preserve ID, name, reports and unrelated entries at capacity across restart', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-update-'));
  try {
    const workspace = new TerminalWorkspace(directory),
      other = new TerminalWorkspace(directory);
    for (let i = 0; i < 19; i++) await workspace.saveScenario(`Plan ${i}`, settings());
    const expected = structuredClone(workspace.data.scenarios[0]),
      s = settings();
    await other.appendRun(
      runRecord({
        ...s,
        preset: 'Supplier delay',
        result: {
          success: true,
          inputs: simulationPlans(s.presetId, s.seed, s.controls, s.products),
          results: [{ name: 'Baseline', summary: {}, timeline: [], inventory: [] }],
        },
      }),
    );
    await other.saveScenario('Other window', settings());
    const before = structuredClone(other.data);
    const proposed = {
      ...settings(),
      presetId: 'cash',
      seed: 53,
      controls: { ...s.controls, startingCash: '37.00' },
      products: [...s.products].reverse(),
    };
    await workspace.updateScenario(expected, proposed);
    const loaded = await new TerminalWorkspace(directory).load();
    Object.assign(
      before.scenarios.find((item) => item.id === expected.id),
      proposed,
    );
    assert.deepEqual(loaded, before);
    assert.equal(loaded.scenarios.length, 20);
    assert.equal((await fs.stat(workspace.file)).mode & 0o777, 0o600);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('stale or removed targets refuse atomically, and retry requires reviewing the latest entry', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-update-conflict-'));
  try {
    const workspace = new TerminalWorkspace(directory),
      other = new TerminalWorkspace(directory);
    await workspace.saveScenario('Original', settings());
    const expected = structuredClone(workspace.data.scenarios[0]);
    await other.renameScenario(expected.id, 'Changed in another window');
    const before = await fs.readFile(workspace.file, 'utf8');
    await assert.rejects(workspace.updateScenario(expected, settings()), /Saved scenario changed/);
    assert.equal(await fs.readFile(workspace.file, 'utf8'), before);
    assert.equal(workspace.data.scenarios[0].name, 'Changed in another window');
    assert.throws(
      () => workspace.updateScenario(workspace.data.scenarios[0], { ...settings(), seed: 0 }),
      /Invalid saved scenario/,
    );
    await workspace.updateScenario(workspace.data.scenarios[0], { ...settings(), seed: 53 });
    assert.equal(workspace.data.scenarios[0].name, 'Changed in another window');
    assert.equal(workspace.data.scenarios[0].seed, 53);
    await other.removeEntry('scenarios', expected.id);
    const removed = await fs.readFile(workspace.file, 'utf8');
    await assert.rejects(workspace.updateScenario(expected, settings()), /no longer exists/);
    assert.equal(await fs.readFile(workspace.file, 'utf8'), removed);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('filtered update review captures independent target and draft, owns keys and requires confirmation', () => {
  const state = panel(),
    opened = key(state, 'S');
  assert.equal(opened.effect, undefined);
  assert.equal(opened.state.input.kind, 'scenario-update');
  assert.equal(opened.state.input.expected.id, 'b');
  assert.equal(opened.state.input.settings.presetId, 'baseline');
  assert.equal(opened.state.input.settings.seed, 42);
  state.planning.startingCash = '39.00';
  state.library.scenarios[1].name = 'Changed';
  assert.equal(opened.state.input.settings.controls.startingCash, '200.00');
  assert.equal(opened.state.input.expected.name, 'Selected');
  for (const text of ['s', 'q', 'w', 'S', 'N'])
    assert.equal(key(opened.state, text).effect, undefined);
  const submitted = key(opened.state, '', 'return');
  assert.equal(submitted.effect, 'update-scenario');
  assert.equal(key(submitted.state, '', 'return').effect, undefined);
  assert.equal(key(opened.state, '', 'escape').state.input, null);
  assert.equal(key(opened.state, '', 'end').state.input.offset, Number.MAX_SAFE_INTEGER);
  assert.ok(paletteCommands('Update selected', panel()).some((command) => command.key === 'S'));
  for (const blocked of [
    { ...state, sim: { running: true } },
    { ...state, panel: 'history' },
    { ...state, panelQuery: 'missing' },
  ]) {
    assert.equal(key(blocked, 'S').state.input, null);
    assert.equal(paletteCommands('Update selected', blocked).length, 0);
  }
  const fromPalette = key(key(key(panel(), ':').state, 'Update selected').state, '', 'return');
  assert.equal(fromPalette.state.input.expected.id, 'b');
});

test('update review shows both exact assumptions and SKU priorities with bounded scrolling after resize', () => {
  const state = panel();
  state.products = settings().products;
  state.planning.startingCash = '37.00';
  const review = key(state, 'S').state.input;
  const text = scenarioUpdateLines(review, 118).join('\n');
  assert.match(text, /SAVED · Supplier delay · seed 77/);
  assert.match(text, /PROPOSED · Steady demand · seed 42/);
  assert.match(text, /USD 10.00 opening cash/);
  assert.match(text, /USD 37.00 opening cash/);
  assert.match(text, /Priority 1 · SECOND/);
  assert.match(text, /Supplier minimum 4 · case pack 2/);
  const narrow = scenarioUpdateLimit(review, 40, 14),
    wide = scenarioUpdateLimit(review, 140, 44);
  assert.ok(narrow > wide);
  const end = render({ ...state, input: { ...review, offset: narrow } }, 40, 14, { color: false });
  assert.match(end, /No simulation runs/);
  const empty = { ...review, settings: { ...review.settings, products: [] } };
  assert.match(scenarioUpdateLines(empty, 118).join('\n'), /COFFEE-250G: stock 12/);
});

async function terminal(t, workspace) {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter(),
    calls = [];
  input.setRawMode = () => {};
  output.columns = 120;
  output.rows = 44;
  let frame = '';
  output.on('data', (chunk) => {
    frame = chunk.toString();
  });
  const backend = {
    close: async () => {},
    request: async (operation) => {
      calls.push(operation);
      if (operation !== 'snapshot') throw new Error('Updating must not run simulations');
      return { counts: {}, errors: [], orders: [], customers: [], inventory: [], payments: [] };
    },
  };
  const done = startTerminal({ input, output, signals, backend, workspace, color: false });
  t.after(async () => {
    input.emit('keypress', '', { ctrl: true, name: 'c' });
    await done;
  });
  await tick();
  const send = (text, name = text) => input.emit('keypress', text, { name });
  for (const text of ['6', 'l', '/', 'Selected']) send(text);
  send('', 'return');
  send('S');
  return { send, output, calls, frame: () => frame };
}

test('app cancellation never persists, loads assumptions or runs a simulation', async (t) => {
  let updates = 0;
  const app = await terminal(t, {
    load: async () => library(),
    updateScenario: async () => {
      updates++;
    },
  });
  assert.match(app.frame(), /UPDATE SCENARIO/);
  app.send('', 'escape');
  assert.match(app.frame(), /Update canceled/);
  assert.equal(updates, 0);
  assert.deepEqual(app.calls, ['snapshot']);
});

test('async confirmed update survives pending redraw and retains target selection after another entry is inserted', async (t) => {
  const updates = [],
    data = library();
  let resolve;
  const app = await terminal(t, {
    load: async () => data,
    updateScenario: (expected, proposed) => {
      updates.push({ expected, proposed });
      return new Promise((done) => {
        resolve = done;
      });
    },
  });
  app.send('', 'return');
  app.send('', 'return');
  app.output.columns = 55;
  app.output.emit('resize');
  assert.match(app.frame(), /Updating scenario/);
  const latest = structuredClone(data);
  latest.scenarios.unshift({ id: 'c', name: 'Other window', ...settings() });
  Object.assign(latest.scenarios[2], updates[0].proposed);
  resolve(latest);
  await tick();
  assert.equal(updates.length, 1);
  assert.equal(updates[0].expected.id, 'b');
  assert.equal(updates[0].proposed.controls.startingCash, '200.00');
  assert.match(app.frame(), /Scenario updated/);
  assert.match(app.frame(), /› Selected/);
  assert.match(app.frame(), /3 \/ 3 entries/);
  assert.deepEqual(app.calls, ['snapshot']);
});

test('app conflict refreshes the library and requires a new review before retrying with the latest target', async (t) => {
  const data = library(),
    updates = [];
  const workspace = {
    data,
    load: async () => data,
    updateScenario: async (expected, proposed) => {
      updates.push({ expected, proposed });
      if (updates.length === 1) {
        workspace.data = structuredClone(data);
        workspace.data.scenarios[1].name = 'Changed elsewhere';
        throw new Error('Saved scenario changed. Esc, then Shift+S to review it again.');
      }
      const latest = structuredClone(workspace.data);
      Object.assign(latest.scenarios[1], proposed);
      return latest;
    },
  };
  const app = await terminal(t, workspace);
  app.send('', 'return');
  await tick();
  assert.match(app.frame(), /Saved scenario changed/);
  app.send('', 'escape');
  app.send('S');
  assert.match(app.frame(), /Target: Changed elsewhere/);
  app.send('', 'return');
  await tick();
  assert.equal(updates[1].expected.name, 'Changed elsewhere');
  assert.match(app.frame(), /Scenario updated/);
  assert.deepEqual(app.calls, ['snapshot']);
});
