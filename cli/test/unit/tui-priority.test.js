import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {
  initialState,
  navigate,
  handleKey,
  records,
  paletteCommands,
  simulationPlans,
} from '../../src/tui/model.js';
import { productEditor, productFromDraft } from '../../src/tui/basket.js';
import { render } from '../../src/tui/render.js';
import { TerminalWorkspace } from '../../src/tui/workspace.js';

const key = (state, text, name = text) => handleKey(state, text, { name });
const products = () =>
  ['FIRST', 'SECOND', 'THIRD'].map((sku) =>
    productFromDraft(sku, {
      ...productEditor({ sku, available: '0', priceExact: '2.00' }).draft,
      unitCost: '1.00',
    }),
  );
const panel = () => ({ ...navigate(initialState(), 5), panel: 'products', products: products() });
const order = (state) => state.products.map((product) => product.sku);

test('priority controls move the actual filtered SKU through the full plan without mutating saved results', () => {
  const original = panel();
  original.sim = {
    ...original.sim,
    products: structuredClone(original.products),
    result: { inputs: simulationPlans('baseline', 42, original.planning, original.products) },
  };
  const resultBefore = structuredClone(original.sim);
  let state = { ...original, panelQuery: 'SECOND' };
  assert.deepEqual(
    records(state).map((p) => p.sku),
    ['SECOND'],
  );
  let moved = key(state, '[');
  assert.equal(moved.effect, undefined);
  state = moved.state;
  assert.deepEqual(order(state), ['SECOND', 'FIRST', 'THIRD']);
  assert.equal(records(state)[state.selected].sku, 'SECOND');
  assert.match(render(state, 55, 20, { color: false }), /1\. SECOND/);
  assert.match(state.notice, /priority 1 of 3/);
  assert.deepEqual(order(original), ['FIRST', 'SECOND', 'THIRD']);
  assert.deepEqual(state.sim, resultBefore);
  assert.equal(key(state, '[').state, state, 'first priority does not wrap');
  state = key(state, ']').state;
  state = key(state, ']').state;
  assert.deepEqual(order(state), ['FIRST', 'THIRD', 'SECOND']);
  assert.match(render(state, 55, 20, { color: false }), /3\. SECOND/);
  assert.equal(records(state)[state.selected].sku, 'SECOND');
  assert.equal(key(state, ']').state, state, 'last priority does not wrap');
  assert.deepEqual(
    simulationPlans('baseline', 42, state.planning, state.products)[0].products.map((p) => p.sku),
    order(state),
  );
  assert.deepEqual(state.sim, resultBefore);
});

test('reordering preserves selection without a filter and owns no input, store or running-job actions', () => {
  let state = { ...panel(), selected: 1 };
  state = key(state, '[').state;
  assert.equal(state.selected, 0);
  assert.equal(records(state)[state.selected].sku, 'SECOND');
  state = key(state, ']').state;
  assert.equal(state.selected, 1);
  assert.equal(records(state)[state.selected].sku, 'SECOND');
  const running = { ...state, sim: { ...state.sim, running: true } };
  for (const char of ['[', ']']) {
    const blocked = key(running, char);
    assert.equal(blocked.effect, undefined);
    assert.equal(blocked.state.products, running.products);
    const empty = { ...state, panelQuery: 'no-such-sku' };
    assert.equal(key(empty, char).state, empty);
    assert.match(render(empty, 55, 20, { color: false }), /No SKUs match this filter/);
  }
  const typing = key(state, '/').state;
  const typed = key(typing, ']').state;
  assert.equal(typed.input.text, ']');
  assert.equal(typed.products, state.products);
  const commands = paletteCommands('purchase priority', state);
  assert.deepEqual(
    commands.map((command) => command.key),
    ['[', ']'],
  );
  assert.equal(paletteCommands('purchase priority', running).length, 0);
  const selected = key(key(key(state, ':').state, 'Move selected SKU earlier').state, '', 'return');
  assert.deepEqual(order(selected.state), ['SECOND', 'FIRST', 'THIRD']);
  assert.equal(selected.effect, undefined);
});

test('saved scenarios retain reordered purchase priority through a workspace restart', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-priority-'));
  try {
    const state = key({ ...panel(), selected: 1 }, '[').state;
    const workspace = new TerminalWorkspace(directory, 'demo');
    await workspace.saveScenario('Priority plan', {
      presetId: 'baseline',
      seed: 42,
      controls: state.planning,
      products: state.products,
    });
    const loaded = await new TerminalWorkspace(directory, 'demo').load();
    assert.deepEqual(
      loaded.scenarios[0].products.map((p) => p.sku),
      ['SECOND', 'FIRST', 'THIRD'],
    );
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});
