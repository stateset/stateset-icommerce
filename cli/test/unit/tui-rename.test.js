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
import { render } from '../../src/tui/render.js';

const tick = () => new Promise((resolve) => setImmediate(resolve));
const settings = () => ({
  presetId: 'delay',
  seed: 77,
  controls: { ...initialState().planning, startingCash: '10.00' },
  products: ['SECOND', 'FIRST'].map((sku) =>
    productFromDraft(sku, {
      ...productEditor({ sku, available: '3', priceExact: '2.00' }).draft,
      unitCost: '1.00',
    }),
  ),
});
const library = () => ({
  scenarios: [
    { id: 'a', name: 'First', ...settings() },
    { id: 'b', name: 'Original second', ...settings() },
  ],
  runs: [],
});
const key = (state, text, name = text) => handleKey(state, text, { name });
const panel = () => ({
  ...navigate(initialState(), 5),
  panel: 'scenarios',
  panelQuery: 'Original',
  library: library(),
});

test('rename preserves identity, reports and latest assumptions at full capacity across workspace restart', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-rename-'));
  try {
    const workspace = new TerminalWorkspace(directory),
      other = new TerminalWorkspace(directory);
    for (let i = 0; i < 19; i++) await workspace.saveScenario(`Plan ${i}`, settings());
    const item = structuredClone(workspace.data.scenarios[0]);
    const s = settings();
    const run = runRecord({
      ...s,
      preset: 'Supplier delay',
      result: {
        success: true,
        inputs: simulationPlans(s.presetId, s.seed, s.controls, s.products),
        results: [{ name: 'Baseline', summary: {}, timeline: [], inventory: [] }],
      },
    });
    await other.appendRun(run);
    await other.update((data) => {
      data.scenarios.find((scenario) => scenario.id === item.id).controls.startingCash = '37.00';
      return data;
    });
    await other.saveScenario('Other window', settings());
    const before = structuredClone(other.data);
    await workspace.renameScenario(item.id, '  Purchasing plan  ');
    const loaded = await new TerminalWorkspace(directory).load();
    const expected = structuredClone(before);
    expected.scenarios.find((scenario) => scenario.id === item.id).name = 'Purchasing plan';
    assert.deepEqual(loaded, expected, 'only the selected saved name changes');
    assert.equal(loaded.scenarios.length, 20);
    assert.equal(
      loaded.scenarios.find((scenario) => scenario.id === item.id).controls.startingCash,
      '37.00',
    );
    assert.equal((await fs.stat(workspace.file)).mode & 0o777, 0o600);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('invalid names, removed IDs and writer locks preserve the saved file and allow retry', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-rename-refuse-'));
  try {
    const workspace = new TerminalWorkspace(directory);
    await workspace.saveScenario('Original', settings());
    const id = workspace.data.scenarios[0].id,
      before = await fs.readFile(workspace.file, 'utf8');
    for (const name of ['', '  ', 'x'.repeat(81), null])
      assert.throws(() => workspace.renameScenario(id, name), /scenario name/);
    assert.throws(() => workspace.renameScenario(undefined, 'New'), /Choose a saved scenario/);
    await assert.rejects(workspace.renameScenario('removed', 'New'), /no longer exists/);
    const lock = path.join(workspace.directory, 'workspace.lock');
    await fs.writeFile(lock, 'another writer');
    await assert.rejects(workspace.renameScenario(id, 'New'), /locked/);
    assert.equal(await fs.readFile(lock, 'utf8'), 'another writer');
    assert.equal(await fs.readFile(workspace.file, 'utf8'), before);
    await fs.unlink(lock);
    await workspace.renameScenario(id, 'New');
    assert.equal((await new TerminalWorkspace(directory).load()).scenarios[0].name, 'New');
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('rename targets the filtered saved entry, is discoverable and respects active runs and owned input', () => {
  const state = panel();
  const opened = key(state, 'N');
  assert.deepEqual(opened.state.input, { kind: 'rename', id: 'b', text: 'Original second' });
  assert.equal(opened.effect, undefined);
  assert.ok(paletteCommands('rename', state).some((command) => command.key === 'N'));
  const selected = key(key(key(state, ':').state, 'Rename selected').state, '', 'return');
  assert.equal(selected.state.input.id, 'b');
  for (const blocked of [
    { ...state, sim: { running: true } },
    { ...state, panelQuery: 'missing' },
    { ...state, panel: 'history' },
    { ...state, view: 2, panel: null },
  ]) {
    assert.equal(key(blocked, 'N').state.input, null);
    assert.equal(paletteCommands('rename', blocked).length, 0);
  }
  const editing = key({ ...state, input: { kind: 'search', text: '' } }, 'N');
  assert.equal(editing.state.input.text, 'N');
  assert.equal(editing.effect, undefined);
  assert.equal(key(opened.state, '', 'escape').state.input, null);
  assert.deepEqual(opened.state.library, state.library);
  assert.match(render(opened.state, 55, 20, { color: false }), /RENAME SCENARIO/);
});

test('rename validates names before submission and prevents duplicate writes while pending', () => {
  const opened = key(panel(), 'N').state;
  const blank = key({ ...opened, input: { ...opened.input, text: ' ' } }, '', 'return');
  assert.equal(blank.effect, undefined);
  assert.match(blank.state.input.error, /1–80/);
  const long = key({ ...opened, input: { ...opened.input, text: 'x'.repeat(81) } }, '', 'return');
  assert.equal(long.effect, undefined);
  const submitted = key(opened, '', 'return');
  assert.equal(submitted.effect, 'rename-scenario');
  for (const [text, name] of [
    ['', 'return'],
    ['q', 'q'],
    ['s', 's'],
    ['', 'escape'],
  ]) {
    const blocked = key(submitted.state, text, name);
    assert.equal(blocked.state, submitted.state);
    assert.equal(blocked.effect, undefined);
  }
  assert.equal(handleKey(submitted.state, '', { ctrl: true, name: 'c' }).effect, 'quit');
  assert.match(render(submitted.state, 55, 20, { color: false }), /Renaming/);
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
      if (operation !== 'snapshot') throw new Error('Rename must not execute a simulation');
      return { counts: {}, errors: [], orders: [], customers: [], inventory: [], payments: [] };
    },
  };
  const done = startTerminal({ input, output, signals, backend, workspace, color: false });
  t.after(async () => {
    input.emit('keypress', '', { ctrl: true, name: 'c' });
    await done;
  });
  await tick();
  const send = (text, name = text, extra = {}) => input.emit('keypress', text, { name, ...extra });
  for (const text of ['6', 'l', '/', 'Original']) send(text);
  send('', 'return');
  send('N');
  send('', 'u', { ctrl: true });
  send('Renamed');
  return { send, calls, frame: () => frame };
}

test('app renames a filtered ID, retains selection after concurrent insert and leaves the draft untouched', async (t) => {
  const data = library(),
    names = [],
    saves = [];
  let resolve;
  const workspace = {
    load: async () => data,
    renameScenario: (id, name) => {
      names.push({ id, name });
      return new Promise((done) => {
        resolve = done;
      });
    },
    saveScenario: async (name, s) => {
      saves.push(s);
      return data;
    },
  };
  const app = await terminal(t, workspace);
  app.send('', 'return');
  app.send('', 'return');
  assert.deepEqual(names, [{ id: 'b', name: 'Renamed' }]);
  const latest = structuredClone(data);
  latest.scenarios.unshift({ id: 'c', name: 'Other window', ...settings() });
  latest.scenarios.find((item) => item.id === 'b').name = 'Renamed';
  resolve(latest);
  await tick();
  assert.match(app.frame(), /› Renamed/);
  assert.match(app.frame(), /3 \/ 3 entries/);
  assert.match(app.frame(), /Scenario renamed/);
  app.send('w');
  app.send('Current draft');
  app.send('', 'return');
  await tick();
  assert.equal(saves[0].presetId, 'baseline');
  assert.equal(saves[0].seed, 42);
  assert.deepEqual(saves[0].products, []);
  assert.deepEqual(app.calls, ['snapshot']);
  assert.equal(data.scenarios[1].name, 'Original second');
});

test('failed rename keeps the dialog, target and typed name available for explicit retry', async (t) => {
  const data = library(),
    attempts = [];
  const workspace = {
    load: async () => data,
    renameScenario: async (id, name) => {
      attempts.push({ id, name });
      if (attempts.length === 1) throw new Error('Workspace is locked by another writer');
      return {
        ...data,
        scenarios: data.scenarios.map((item) => (item.id === id ? { ...item, name } : item)),
      };
    },
  };
  const app = await terminal(t, workspace);
  app.send('', 'return');
  await tick();
  assert.match(app.frame(), /RENAME SCENARIO/);
  assert.match(app.frame(), /Workspace is locked/);
  app.send('', 'u', { ctrl: true });
  app.send('Retry name');
  app.send('', 'return');
  await tick();
  assert.deepEqual(attempts, [
    { id: 'b', name: 'Renamed' },
    { id: 'b', name: 'Retry name' },
  ]);
  assert.match(app.frame(), /› Retry name/);
  assert.deepEqual(app.calls, ['snapshot']);
});
