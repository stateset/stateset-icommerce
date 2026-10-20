import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {
  PRODUCT_FIELDS,
  productEditor,
  productFromDraft,
  planningProducts,
} from '../../src/tui/basket.js';
import { initialState, navigate, handleKey, simulationPlans } from '../../src/tui/model.js';
import { TerminalWorkspace, runRecord, useRunAssumptions } from '../../src/tui/workspace.js';
import { importedScenario } from '../../src/tui/import.js';
import { render, importReviewLines, scenarioUpdateLines } from '../../src/tui/render.js';

const legacy = () => ({
  sku: 'PACK',
  initialStock: 0,
  unitPrice: '2.00',
  unitCost: '1.00',
  reorderPoint: 0,
  targetStock: 3,
  leadTimeDays: 1,
  demand: { min: 0, max: 0 },
  purchasing: { minimumOrderQuantity: 5, orderMultiple: 4 },
});
const configured = () =>
  productFromDraft('PACK', {
    ...productEditor(legacy()).draft,
    reviewIntervalDays: '3',
    maximumInventoryPosition: '6',
  });
const settings = () => ({
  presetId: 'delay',
  seed: 42,
  controls: { ...initialState().planning, days: 2 },
  products: [configured()],
});
const record = (s) =>
  runRecord({
    ...s,
    preset: 'Supplier delay',
    result: {
      success: true,
      inputs: simulationPlans(s.presetId, s.seed, s.controls, s.products),
      results: [{ name: 'Baseline', summary: {}, inventory: [], timeline: [] }],
    },
  });

test('optional SKU rules default to daily and no cap, enforce bounds, and can remove an existing cap', () => {
  const draft = productEditor(legacy()).draft;
  assert.equal(draft.reviewIntervalDays, '1');
  assert.equal(draft.maximumInventoryPosition, '');
  assert.deepEqual(productFromDraft('PACK', draft).purchasing, legacy().purchasing);
  const oldDraft = { ...draft };
  delete oldDraft.reviewIntervalDays;
  delete oldDraft.maximumInventoryPosition;
  assert.deepEqual(productFromDraft('PACK', oldDraft).purchasing, legacy().purchasing);
  assert.deepEqual(configured().purchasing, {
    minimumOrderQuantity: 5,
    orderMultiple: 4,
    reviewIntervalDays: 3,
    maximumInventoryPosition: 6,
  });
  assert.equal(
    productFromDraft('PACK', {
      ...draft,
      reviewIntervalDays: '60',
      maximumInventoryPosition: '1000000',
    }).purchasing.maximumInventoryPosition,
    1000000,
  );
  for (const value of ['0', '61', '1.5', '-1', '', '1e1', null, false])
    assert.throws(
      () => productFromDraft('PACK', { ...draft, reviewIntervalDays: value }),
      /Review interval/,
    );
  for (const value of ['0', '1000001', '1.5', '-1', ' ', '1e3', null, false])
    assert.throws(
      () => productFromDraft('PACK', { ...draft, maximumInventoryPosition: value }),
      /Position cap/,
    );
  const removed = productFromDraft('PACK', {
    ...productEditor(configured()).draft,
    reviewIntervalDays: '1',
    maximumInventoryPosition: '',
  });
  assert.deepEqual(removed.purchasing, legacy().purchasing);
});

test('normalized SKU policies retain every supported rule, reject unsupported terms and preserve them across comparisons', () => {
  const products = [configured()];
  assert.deepEqual(planningProducts(products), products);
  for (const key of ['callback', 'maximumStock', 'reviewDay'])
    assert.throws(
      () =>
        planningProducts([
          { ...configured(), purchasing: { ...configured().purchasing, [key]: 1 } },
        ]),
      /Unsupported/,
    );
  for (const preset of ['baseline', 'spike', 'delay', 'cash']) {
    const plans = simulationPlans(preset, 42, initialState().planning, products);
    assert.ok(
      plans.every(
        (plan) =>
          JSON.stringify(plan.products[0].purchasing) === JSON.stringify(products[0].purchasing),
      ),
    );
  }
});

test('filtered SKU editors own the rule fields, retain valid drafts on failure and cancel without changing saved inputs', () => {
  const products = [{ ...legacy(), sku: 'FIRST' }, configured()];
  const original = {
    ...navigate(initialState(), 5),
    panel: 'products',
    panelQuery: 'PACK',
    products,
    sim: {
      ...initialState().sim,
      result: { inputs: simulationPlans('baseline', 42, initialState().planning, products) },
    },
  };
  const before = structuredClone(original);
  let state = handleKey(original, 'e', {}).state;
  assert.equal(state.input.index, 1);
  const index = PRODUCT_FIELDS.findIndex(({ key }) => key === 'reviewIntervalDays');
  for (let i = 0; i < index; i++) state = handleKey(state, '', { name: 'tab' }).state;
  assert.match(render(state, 20, 8, { color: false }), /Review interval/);
  assert.match(render(state, 140, 44, { color: false }), /review on day zero/);
  state = handleKey(state, '', { name: 'tab' }).state;
  state = handleKey(state, '', { name: 'u', ctrl: true }).state;
  state = handleKey(state, '0', {}).state;
  state = handleKey(state, '', { name: 'return' }).state;
  assert.match(state.input.error, /Position cap/);
  assert.deepEqual(state.products, before.products);
  assert.match(render(state, 20, 8, { color: false }), /Position cap/);
  assert.equal(handleKey(state, 's', {}).effect, undefined);
  assert.deepEqual(handleKey(state, '', { name: 'escape' }).state.products, before.products);
  state = handleKey(state, '', { name: 'u', ctrl: true }).state;
  const saved = handleKey(state, '', { name: 'return' });
  assert.equal(saved.effect, undefined);
  assert.equal(saved.state.input, null);
  assert.equal(saved.state.products[1].purchasing.reviewIntervalDays, 3);
  assert.ok(!Object.hasOwn(saved.state.products[1].purchasing, 'maximumInventoryPosition'));
  assert.deepEqual(saved.state.products[0], before.products[0]);
  assert.deepEqual(saved.state.sim, before.sim);
  assert.deepEqual(original, before);
});

test('rule-aware reviews, workspace restart, imported assumptions and run drafts preserve policies and original reports', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-purchasing-rules-'));
  try {
    const workspace = new TerminalWorkspace(directory);
    const s = settings();
    await workspace.saveScenario('Review and capacity', s);
    const expected = structuredClone(workspace.data.scenarios[0]);
    const proposed = structuredClone(s);
    proposed.products = [
      productFromDraft('PACK', {
        ...productEditor(configured()).draft,
        reviewIntervalDays: '1',
        maximumInventoryPosition: '',
      }),
    ];
    const text = scenarioUpdateLines({ expected, settings: proposed }, 160).join('\n');
    assert.match(text, /Purchase review interval 3 day\(s\)/);
    assert.match(text, /Maximum inventory position 6/);
    assert.match(text, /Purchase review interval 1 day\(s\)/);
    assert.match(text, /Maximum inventory position No cap/);
    await workspace.updateScenario(expected, proposed);
    const run = record(s),
      before = structuredClone(run);
    await workspace.appendRun(run);
    const loaded = await new TerminalWorkspace(directory).load();
    assert.deepEqual(loaded.scenarios[0].products[0].purchasing, legacy().purchasing);
    const draft = useRunAssumptions(navigate(initialState(), 5), loaded.runs[0]);
    assert.deepEqual(draft.products[0].purchasing, configured().purchasing);
    assert.deepEqual(
      simulationPlans(draft.selectedPreset, draft.seed, draft.planning, draft.products),
      run.result.inputs,
    );
    const preview = importedScenario({ format: 'stateset-tui-run', version: 1, ...run });
    assert.match(importReviewLines(preview, 160).join('\n'), /Purchase review interval 3 day\(s\)/);
    assert.match(importReviewLines(preview, 160).join('\n'), /Maximum inventory position 6/);
    const tampered = structuredClone(run);
    tampered.products[0].purchasing.reviewIntervalDays = 4;
    assert.throws(
      () => importedScenario({ format: 'stateset-tui-run', version: 1, ...tampered }),
      /differ from executed inputs/,
    );
    draft.products[0].purchasing.maximumInventoryPosition = 9;
    assert.deepEqual(loaded.runs[0], before);
    assert.deepEqual(run, before);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('legacy SKU run serialization and digests survive loading, import and editing with default rules', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-purchasing-legacy-'));
  try {
    const s = { ...settings(), products: [legacy()] };
    const run = record(s),
      before = structuredClone(run);
    // These products were already supported before the new editor fields.
    assert.deepEqual(run.result.inputs[0].products[0].purchasing, {
      minimumOrderQuantity: 5,
      orderMultiple: 4,
    });
    const workspace = new TerminalWorkspace(directory);
    await workspace.appendRun(run);
    const loaded = await new TerminalWorkspace(directory).load();
    const preview = importedScenario({ format: 'stateset-tui-run', version: 1, ...loaded.runs[0] });
    const products = preview.settings.products.map((item) =>
      productFromDraft(item.sku, productEditor(item).draft),
    );
    assert.equal(
      JSON.stringify(simulationPlans(s.presetId, s.seed, s.controls, products)),
      JSON.stringify(before.result.inputs),
    );
    assert.deepEqual(loaded.runs[0], before);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});
