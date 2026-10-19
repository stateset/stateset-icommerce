import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { importedScenario, readScenarioExport } from '../../src/tui/import.js';
import { TerminalWorkspace, runRecord } from '../../src/tui/workspace.js';
import {
  initialState,
  navigate,
  handleKey,
  records,
  simulationPlans,
  paletteCommands,
} from '../../src/tui/model.js';
import { render } from '../../src/tui/render.js';
import { startTerminal } from '../../src/tui/app.js';

const settings = () => ({
  presetId: 'spike',
  seed: 42,
  controls: initialState().planning,
  products: [],
});
const exportValue = () => {
  const s = settings();
  return {
    format: 'stateset-tui-run',
    version: 1,
    ...runRecord({
      ...s,
      preset: 'Demand spike',
      result: {
        success: true,
        inputs: simulationPlans(s.presetId, s.seed, s.controls),
        results: [],
      },
    }),
  };
};

test('run import validates exact inputs and digest, and discards claimed results', () => {
  const source = exportValue();
  source.result.results = [{ summary: { endingCash: '999999999.99' } }];
  const preview = importedScenario(source, '/private/export.json');
  assert.equal(preview.filename, 'export.json');
  assert.deepEqual(preview.settings, settings());
  assert.equal(preview.result, undefined);
  assert.throws(() => importedScenario({ ...source, seed: 43 }), /differ/);
  assert.throws(() => importedScenario({ ...source, inputDigest: '0'.repeat(64) }), /digest/);
  assert.throws(() => importedScenario({ ...source, kernelVersion: 999 }), /Unsupported/);
});

test('import reads bounded regular JSON files and rejects symlinks, directories and malformed data', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-import-'));
  try {
    const file = path.join(directory, 'run.json');
    await fs.writeFile(file, JSON.stringify(exportValue()));
    assert.equal((await readScenarioExport(file)).settings.seed, 42);
    const link = path.join(directory, 'link.json');
    await fs.symlink(file, link);
    await assert.rejects(readScenarioExport(link), /ELOOP|symlink/);
    await assert.rejects(readScenarioExport(directory), /regular/);
    await fs.writeFile(file, '{broken');
    await assert.rejects(readScenarioExport(file), /valid JSON/);
    const handle = await fs.open(file, 'w');
    await handle.truncate(8 * 1024 * 1024 + 1);
    await handle.close();
    await assert.rejects(readScenarioExport(file), /8 MiB/);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('import preview owns keys, supports review scrolling and never dispatches execution', () => {
  const state = {
    ...navigate(initialState(), 5),
    input: { kind: 'import-review', preview: importedScenario(exportValue()), offset: 0 },
  };
  assert.equal(handleKey(state, 's', {}).effect, undefined);
  assert.equal(handleKey(state, 'q', {}).effect, undefined);
  assert.equal(handleKey(state, '', { name: 'return' }).effect, 'apply-import');
  assert.equal(handleKey(state, '', { name: 'end' }).state.input.offset, Number.MAX_SAFE_INTEGER);
  assert.match(render(state, 55, 20, { color: false }), /IMPORT PREVIEW/);
  assert.match(
    render({ ...state, input: { ...state.input, offset: 999 } }, 55, 20, { color: false }),
    /separate actions/,
  );
  assert.equal(handleKey(state, '', { name: 'escape' }).state.input, null);
});

test('library filters locally, cancels previews and does not read the store', () => {
  let state = {
    ...navigate(initialState(), 5),
    panel: 'scenarios',
    library: {
      runs: [],
      scenarios: [
        { id: 'a', name: 'Demand', ...settings() },
        { id: 'b', name: 'Supplier', ...settings() },
      ],
    },
  };
  state = handleKey(state, '/', {}).state;
  state = handleKey(state, 'Supplier', {}).state;
  assert.equal(records(state).length, 1);
  const accepted = handleKey(state, '', { name: 'return' });
  assert.equal(accepted.effect, undefined);
  state = accepted.state;
  assert.equal(state.panelQuery, 'Supplier');
  state = handleKey(state, '', { name: 'escape' }).state;
  assert.equal(state.panel, 'scenarios');
  assert.equal(records(state).length, 2);
});

test('filtered SKU editing and removal address the actual product, with undo metadata', () => {
  const p = {
    initialStock: 1,
    unitPrice: '1.00',
    unitCost: '0.25',
    demand: { min: 1, max: 2 },
    reorderPoint: 2,
    targetStock: 3,
    leadTimeDays: 1,
  };
  const state = {
    ...navigate(initialState(), 5),
    panel: 'products',
    panelQuery: 'SECOND',
    products: [
      { ...p, sku: 'FIRST' },
      { ...p, sku: 'SECOND' },
    ],
  };
  assert.equal(handleKey(state, 'e', {}).state.input.index, 1);
  const removed = handleKey(state, 'd', {}).state;
  assert.equal(removed.products[0].sku, 'FIRST');
  assert.equal(removed.undoRemoval.index, 1);
  assert.equal(handleKey(removed, 'U', {}).effect, 'undo-removal');
});

test('view changes preserve the chosen preset and filtered lab selection names the actual scenario', () => {
  let state = handleKey(navigate(initialState(), 5), 'j', {}).state;
  assert.equal(state.selectedPreset, 'spike');
  state = handleKey(state, '3', {}).state;
  assert.equal(state.selectedPreset, 'spike');
  state = handleKey(state, '6', {}).state;
  assert.equal(state.selected, 1);
  state.query = 'Supplier';
  state.selected = 0;
  assert.equal(handleKey(state, 's', {}).state.selectedPreset, 'delay');
});

test('committed lab filters update the preset used by save before any execution', () => {
  let state = navigate(initialState(), 5);
  state = handleKey(state, '/', {}).state;
  state = handleKey(state, 'Supplier', {}).state;
  state = handleKey(state, '', { name: 'return' }).state;
  assert.equal(state.selectedPreset, 'delay');
  state = handleKey(state, 'w', {}).state;
  assert.match(render(state, 77, 24, { color: false }), /Saves delay/);
});

test('contextual palette exposes import and library actions through fixed keys', () => {
  let state = navigate(initialState(), 5);
  assert.ok(paletteCommands('import', state).some((c) => c.key === 'i'));
  state.input = { kind: 'palette', text: 'import', selected: 0 };
  const next = handleKey(state, '', { name: 'return' });
  assert.equal(next.state.input.kind, 'import');
  assert.equal(next.effect, undefined);
});

test('removed library entries can be restored without replacing other saved entries', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-undo-'));
  try {
    const workspace = new TerminalWorkspace(directory);
    await workspace.saveScenario('Original', settings());
    const item = workspace.data.scenarios[0];
    await workspace.removeEntry('scenarios', item.id);
    await workspace.saveScenario('Other window', settings());
    const data = await workspace.restoreEntry('scenarios', item);
    assert.deepEqual(
      data.scenarios.map((s) => s.name),
      ['Original', 'Other window'],
    );
    await assert.rejects(workspace.restoreEntry('scenarios', item), /already restored/);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('a delayed snapshot cannot reset a simulation selection or change the dispatched preset', async (t) => {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter();
  input.setRawMode = () => {};
  output.columns = 77;
  output.rows = 44;
  output.resume();
  let resolveSnapshot;
  const calls = [];
  const backend = {
    close: async () => {},
    request: async (operation, payload) => {
      if (operation === 'snapshot')
        return new Promise((resolve) => {
          resolveSnapshot = resolve;
        });
      calls.push(payload);
      throw new Error('Fixture refuses execution');
    },
  };
  const done = startTerminal({ input, output, signals, backend, color: false });
  t.after(async () => {
    input.emit('keypress', 'q', { name: 'q' });
    await done;
  });
  input.emit('keypress', '6', {});
  input.emit('keypress', 'j', {});
  resolveSnapshot({ counts: {}, errors: [], inventory: [] });
  await new Promise((resolve) => setImmediate(resolve));
  input.emit('keypress', 's', {});
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(calls[0].preset, 'spike');
});
