import assert from 'node:assert/strict';
import { test } from 'node:test';
import { recordedFirstStockout } from '../../src/tui/report.js';
import { resultLines, render, panelLimit } from '../../src/tui/render.js';
import { initialState, navigate } from '../../src/tui/model.js';
import { width } from '../../src/tui/text.js';

const fixture = () => ({
  name: 'Baseline',
  days: 2,
  currency: 'USD',
  summary: { demanded: 8, sold: 3, lostSales: 5 },
  inventory: [{ sku: 'FIRST', sold: 3, lostSales: 5, available: 0, onOrder: 7 }],
  timeline: [],
  inventoryFlow: {
    firstStockout: { day: 2, sku: 'FIRST', demanded: 8, sold: 3, lostSales: 5 },
    daily: [],
  },
  outstandingPurchases: [
    {
      purchase: 'purchase-1',
      sku: 'FIRST',
      quantity: 3,
      orderedQuantity: 5,
      receivedQuantity: 2,
      arrivalDay: 4,
    },
    {
      purchase: 'purchase-2',
      sku: 'FIRST',
      quantity: 4,
      orderedQuantity: 4,
      receivedQuantity: 0,
      arrivalDay: 5,
    },
  ],
});
const stateFor = (...reports) => ({
  ...navigate(initialState(), 5),
  panel: 'results',
  planning: { ...initialState().planning, days: 30, initialStock: 500 },
  sim: { preset: 'Supplier delay', seed: 42, result: { results: reports, inputs: [] } },
});

test('first stockout uses recorded unmet demand, keeps engine tie choice and separates absence from missing evidence', () => {
  const report = fixture();
  const before = structuredClone(report);
  assert.equal(recordedFirstStockout(report), report.inventoryFlow.firstStockout);
  assert.equal(recordedFirstStockout(report).sku, 'FIRST');
  assert.deepEqual(report, before);
  report.inventoryFlow.firstStockout = null;
  assert.equal(
    recordedFirstStockout(report),
    null,
    'zero ending availability alone is not unmet demand',
  );
  delete report.inventoryFlow;
  assert.equal(recordedFirstStockout(report), undefined);
  for (const first of [
    undefined,
    {},
    [],
    { ...before.inventoryFlow.firstStockout, day: 0 },
    { ...before.inventoryFlow.firstStockout, day: 3 },
    { ...before.inventoryFlow.firstStockout, lostSales: 0 },
    { ...before.inventoryFlow.firstStockout, sold: 2 },
    { ...before.inventoryFlow.firstStockout, demanded: '8' },
    { ...before.inventoryFlow.firstStockout, sku: '' },
  ]) {
    report.inventoryFlow = { firstStockout: first };
    assert.equal(recordedFirstStockout(report), undefined);
  }
  delete report.days;
  report.inventoryFlow = { firstStockout: null };
  assert.equal(recordedFirstStockout(report), undefined);
});

test('risk summaries retain per-plan stockouts and separate same-SKU partial receipts without changing reports', () => {
  const baseline = fixture();
  const candidate = {
    ...fixture(),
    name: 'Daily reviews',
    inventoryFlow: { firstStockout: null },
    outstandingPurchases: [],
  };
  const before = structuredClone([baseline, candidate]);
  const text = resultLines(stateFor(baseline, candidate), 180).join('\n');
  assert.match(text, /Baseline · INVENTORY RISK/);
  assert.match(text, /First unmet-demand stockout: day 2 · FIRST/);
  assert.match(text, /Demanded 8 · sold 3 · lost 5 units/);
  assert.match(text, /purchase-1 · FIRST/);
  assert.match(text, /Remaining units 3 · ordered 5 · received 2/);
  assert.match(text, /Expected next receipt day 4 · 2 day\(s\) beyond horizon/);
  assert.match(text, /purchase-2 · FIRST/);
  assert.match(text, /Remaining units 4 · ordered 4 · received 0/);
  assert.match(text, /Expected next receipt day 5 · 3 day\(s\) beyond horizon/);
  const candidateText = text.split('Daily reviews · INVENTORY RISK')[1];
  assert.match(candidateText, /No unmet-demand stockout recorded/);
  assert.match(candidateText, /No outstanding purchases recorded at horizon/);
  assert.doesNotMatch(candidateText, /purchase-[12]/);
  assert.match(text, /inbound, not available stock/);
  assert.match(text, /Receipt dates are scenario assumptions/);
  assert.doesNotMatch(text, /day 30|500/);
  assert.deepEqual([baseline, candidate], before);
});

test('missing and incomplete pending receipts remain unavailable, preserve known fields and do not invent lateness', () => {
  const report = fixture();
  delete report.inventoryFlow;
  delete report.outstandingPurchases;
  let text = resultLines(stateFor(report), 180).join('\n');
  assert.match(text, /First unmet-demand stockout: Unavailable/);
  assert.match(text, /Outstanding purchase evidence: Unavailable/);
  assert.doesNotMatch(text, /No unmet-demand|No outstanding/);
  report.outstandingPurchases = [
    null,
    [],
    { purchase: 'older-purchase', sku: 'FIRST', quantity: 3, arrivalDay: 1 },
  ];
  text = resultLines(stateFor(report), 180).join('\n');
  assert.match(text, /Invalid outstanding purchase entry/);
  assert.match(text, /older-purchase · FIRST/);
  assert.match(text, /Remaining units 3 · ordered Unavailable · received Unavailable/);
  assert.match(text, /Expected next receipt day 1 · at\/before horizon; inspect raw report/);
  assert.doesNotMatch(text, /overdue|late delivery/);
  report.outstandingPurchases[2].arrivalDay = 0;
  text = resultLines(stateFor(report), 180).join('\n');
  assert.match(text, /Expected next receipt day Unavailable/);
  assert.doesNotMatch(text, /Expected next receipt day 0/);
});

test('risk evidence is reachable by compact scrolling, strips terminal control text and retains exportable report contents', () => {
  const report = fixture();
  report.inventoryFlow.firstStockout.sku = 'FIRST\x1b]52;c;clipboard\x07';
  report.outstandingPurchases[0].purchase = 'purchase-1\x1b[31m';
  const before = JSON.stringify(report);
  const state = stateFor(report);
  for (const [columns, rows] of [
    [20, 8],
    [40, 14],
    [140, 44],
  ]) {
    const frames = [];
    for (let panelOffset = 0; panelOffset <= panelLimit(state, columns, rows); panelOffset++) {
      const frame = render({ ...state, panelOffset }, columns, rows, { color: false });
      assert.ok(frame.split('\n').length <= rows - 1);
      assert.ok(frame.split('\n').every((line) => width(line) <= columns - 1));
      assert.doesNotMatch(frame, /\x1b|clipboard/);
      frames.push(frame);
    }
    assert.match(frames.join('\n'), /INVENTORY RISK/);
    assert.match(frames.join('\n'), /purchase-1/);
    assert.match(frames.join('\n'), /received 2/);
    assert.equal(JSON.stringify(report), before);
  }
});
