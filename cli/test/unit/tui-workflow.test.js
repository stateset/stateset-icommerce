import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { TerminalWorkspace, runRecord, digestInputs } from '../../src/tui/workspace.js';
import { productEditor, productFromDraft, planningProducts } from '../../src/tui/basket.js';
import { initialState, navigate, handleKey, simulationPlans } from '../../src/tui/model.js';
import { resultLines, render, panelLimit } from '../../src/tui/render.js';
import { inventoryPlanComparisonSchema } from '../../src/simulation/schema.js';

const product = () =>
  productFromDraft('TEST-SKU', {
    ...productEditor({ sku: 'TEST-SKU', available: '7', priceExact: '12.50' }).draft,
    unitCost: '3.75',
  });
const settings = () => ({
  presetId: 'delay',
  seed: 42,
  controls: initialState().planning,
  products: [product()],
});
const successful = () => {
  const s = settings();
  return {
    ...s,
    preset: 'Supplier delay',
    result: {
      success: true,
      inputs: simulationPlans(s.presetId, s.seed, s.controls, s.products),
      results: [
        {
          name: 'Baseline',
          currency: 'USD',
          days: 7,
          summary: {
            sold: 1,
            demanded: 2,
            lostSales: 1,
            endingCash: '9007199254740993.01',
            purchasingSpend: '3.75',
            grossMargin: '8.75',
          },
          timeline: [],
          inventory: [],
        },
      ],
    },
  };
};

test('selected SKUs require explicit cost and bounded whole-unit terms', () => {
  const editor = productEditor({ sku: 'TEST-SKU', available: '7', priceExact: '12.50' });
  assert.equal(editor.draft.unitCost, '');
  assert.throws(() => productFromDraft(editor.sku, editor.draft), /Purchase unit cost/);
  assert.equal(product().initialStock, 7);
  assert.equal(product().unitPrice, '12.50');
  assert.throws(
    () => productFromDraft(editor.sku, { ...editor.draft, unitCost: '1.00', initialStock: '0.5' }),
    /Opening stock/,
  );
  assert.throws(
    () => productFromDraft(editor.sku, { ...editor.draft, unitCost: '1.00', demandMax: '1' }),
    /Demand maximum/,
  );
  assert.throws(() => planningProducts([product(), product()]), /unique/);
});

test('three-SKU comparisons fit native admission and apply shocks to every SKU', () => {
  const products = ['A', 'B', 'C'].map((sku) => ({ ...product(), sku }));
  const plans = simulationPlans('spike', 42, { ...initialState().planning, days: 30 }, products);
  inventoryPlanComparisonSchema.parse(plans);
  assert.equal(plans[1].products.length, 3);
  assert.ok(plans[1].products.every((p) => p.demand.max === 15));
  assert.ok(plans[0].products.every((p) => p.demand.max === 5));
  assert.throws(
    () => simulationPlans('spike', 42, undefined, [...products, { ...product(), sku: 'D' }]),
    /at most 3/,
  );
});

test('comparison outcomes and executed assumptions remain reachable in a short pane', () => {
  let state = { ...navigate(initialState(), 5), sim: successful(), panel: 'results' };
  const lines = resultLines(state, 52);
  assert.ok(lines.some((line) => line.includes('9007199254740993.01')));
  const top = render(state, 55, 20, { color: false });
  assert.match(top, /COMPARISON/);
  state = handleKey(state, '', { name: 'end' }).state;
  state.panelOffset = Math.min(state.panelOffset, panelLimit(state, 55, 20));
  assert.match(render(state, 55, 20, { color: false }), /Conditional projections/);
  state.detail = true;
  assert.equal(handleKey(state, '', { name: 'end' }).effect, 'detail-end');
  assert.equal(handleKey(state, '', { name: 'escape' }).state.panel, 'results');
});

test('typing a save/export path owns keys and cannot launch actions', () => {
  let state = navigate(initialState(), 5);
  state = handleKey(state, 'w', {}).state;
  for (const c of 'q6/;cash') {
    const next = handleKey(state, c, {});
    assert.equal(next.effect, undefined);
    state = next.state;
  }
  const accepted = handleKey(state, '', { name: 'return' });
  assert.equal(accepted.effect, 'save');
  assert.equal(accepted.state.pendingText, 'q6/;cash');
  state.sim.running = true;
  assert.equal(handleKey({ ...state, input: null }, 'c', {}).effect, 'cancel');
});

test('store-wide query and sort effects reset only the active page', () => {
  let state = navigate(initialState(), 2);
  state.pages.inventory = 4;
  state = handleKey(state, '/', {}).state;
  state = handleKey(state, 'rare-sku', {}).state;
  const accepted = handleKey(state, '', { name: 'return' });
  assert.equal(accepted.effect, 'refresh');
  assert.equal(accepted.state.pages.inventory, 0);
  assert.equal(accepted.state.queries.inventory, 'rare-sku');
  assert.equal(handleKey(accepted.state, 'o', {}).state.sorts.inventory, 1);
  assert.equal(handleKey(accepted.state, '', { name: 'escape' }).state.queries.inventory, '');
});

test('comparison panels yield to save, export and command palette prompts', () => {
  const state = { ...navigate(initialState(), 5), sim: successful(), panel: 'results' };
  for (const [key, label] of [
    ['w', 'SAVE SCENARIO'],
    ['x', 'EXPORT RUN'],
    [':', 'COMMAND PALETTE'],
  ]) {
    const editing = handleKey(state, key, {}).state;
    assert.match(render(editing, 55, 20, { color: false }), new RegExp(label));
  }
});

test('leaving related views refreshes ordinary lists and detail dismissal clears focused records', () => {
  const state = {
    ...navigate(initialState(), 4),
    relation: { field: 'payments', column: 'order_id', id: 'order' },
    detail: false,
  };
  assert.equal(handleKey(state, '2', {}).effect, 'refresh');
  const focused = {
    ...state,
    detail: true,
    focusRecord: { id: 'linked' },
    focusTarget: { domain: 'orders', id: 'linked' },
  };
  const dismissed = handleKey(focused, '', { name: 'escape' }).state;
  assert.equal(dismissed.focusRecord, null);
  assert.equal(dismissed.focusTarget, null);
});

test('workspace survives restart, isolates source stores and preserves exact export values', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-workspace-test-'));
  try {
    const a = new TerminalWorkspace(root, 'demo');
    await a.load();
    await a.saveScenario('Purchasing plan', settings());
    const record = runRecord(successful());
    await a.appendRun(record);
    const b = new TerminalWorkspace(root, 'demo');
    const data = await b.load();
    assert.equal(data.scenarios[0].products[0].unitCost, '3.75');
    assert.equal(data.runs[0].inputDigest, digestInputs(record.result.inputs));
    assert.equal((await fs.stat(a.file)).mode & 0o777, 0o600);
    assert.equal(
      (await new TerminalWorkspace(root, '/tmp/another-store.db').load()).runs.length,
      0,
    );
    const destination = path.join(root, 'report.json');
    await b.exportRun(destination, record);
    assert.equal(
      JSON.parse(await fs.readFile(destination, 'utf8')).result.results[0].summary.endingCash,
      '9007199254740993.01',
    );
    await assert.rejects(b.exportRun(destination, record), { code: 'EEXIST' });
    await a.saveScenario('Second window', settings());
    assert.equal(
      (await b.saveScenario('Third window', settings())).scenarios.length,
      3,
      'reload under lock prevents stale session overwrites',
    );
    await b.removeEntry('scenarios', b.data.scenarios[0].id);
    assert.equal(b.data.scenarios.length, 2);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('workspace refuses corruption and concurrent writers without replacing original data', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-workspace-errors-'));
  try {
    const a = new TerminalWorkspace(root);
    await a.saveScenario('Original', settings());
    const lock = path.join(a.directory, 'workspace.lock');
    await fs.writeFile(lock, 'busy');
    const before = await fs.readFile(a.file);
    await assert.rejects(a.saveScenario('Must not overwrite', settings()), /locked/);
    assert.deepEqual(await fs.readFile(a.file), before);
    await fs.unlink(lock);
    await fs.writeFile(a.file, 'interrupted JSON');
    const b = new TerminalWorkspace(root);
    await assert.rejects(b.load(), /Original file retained/);
    await assert.rejects(b.saveScenario('No overwrite', settings()), /unreadable/);
    assert.equal(await fs.readFile(a.file, 'utf8'), 'interrupted JSON');
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
