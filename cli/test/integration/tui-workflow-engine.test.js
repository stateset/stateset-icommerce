import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash, randomUUID } from 'node:crypto';
import Database from 'better-sqlite3';
import { TerminalBackend } from '../../src/tui/backend.js';
import { DEFAULT_PLANNING } from '../../src/tui/planning.js';
import { initialState, navigate, handleKey } from '../../src/tui/model.js';
import { resultLines, timelineLines } from '../../src/tui/render.js';
import { comparisonEvidence, cashMovement, timelineEntries } from '../../src/tui/report.js';
import { executeInventoryPlan } from '../../src/simulation/planning.js';

test(
  'full inventory, cross-page search, related records, multi-SKU execution and owned cancellation',
  { timeout: 360000 },
  async () => {
    const backend = new TerminalBackend();
    try {
      const initial = await backend.request('snapshot');
      assert.deepEqual(initial.errors, []);
      assert.equal(initial.counts.inventory, 4);
      assert.ok(
        initial.inventory.some(
          (r) =>
            r.sku === 'COFFEE-250G' && typeof r.available === 'string' && r.priceExact === '12.50',
        ),
      );
      const source = path.join(await backend.directory, 'demo.db');
      const writer = new Database(source);
      try {
        const insert = writer.prepare(
          'INSERT INTO customers (id,email,first_name,last_name,created_at,updated_at) VALUES (?,?,?,?,?,?)',
        );
        writer.transaction(() => {
          for (let i = 0; i < 60; i++)
            insert.run(
              randomUUID(),
              i === 59 ? 'needle-unique@example.test' : `fixture-${i}@example.test`,
              'Fixture',
              String(i),
              i === 59 ? '1800-01-01 00:00:00' : '1900-01-01 00:00:00',
              '1900-01-01 00:00:00',
            );
        })();
      } finally {
        writer.close();
      }
      const before = createHash('sha256')
        .update(await fs.readFile(source))
        .digest('hex');
      const firstPage = await backend.request('snapshot');
      assert.equal(firstPage.customers.length, 50);
      assert.ok(!firstPage.customers.some((c) => c.email === 'needle-unique@example.test'));
      const search = await backend.request('snapshot', {
        queries: { customers: 'needle-unique' },
        sorts: { customers: 1 },
      });
      assert.deepEqual(search.errors, []);
      assert.equal(search.matched.customers, 1);
      assert.equal(search.customers[0].email, 'needle-unique@example.test');
      const literal = await backend.request('snapshot', { queries: { customers: '%' } });
      assert.equal(literal.matched.customers, 0, 'SQL wildcard characters are literal user text');
      assert.deepEqual(literal.customers, []);
      const order = initial.orders[0];
      const related = await backend.request('snapshot', {
        relation: { field: 'payments', column: 'order_id', id: order.id },
        focus: { domain: 'orders', id: order.id },
      });
      assert.equal(related.focusRecord.id, order.id);
      assert.ok(related.payments.length > 0);
      assert.ok(related.payments.every((payment) => payment.orderId === order.id));
      const messages = [];
      const product = {
        initialStock: 5,
        unitPrice: '4.00',
        unitCost: '1.25',
        demand: { min: 2, max: 3 },
        reorderPoint: 3,
        targetStock: 8,
        leadTimeDays: 1,
        purchasing: { minimumOrderQuantity: 2, orderMultiple: 2 },
      };
      const run = await backend.request(
        'simulate',
        {
          preset: 'delay',
          seed: 42,
          controls: { ...DEFAULT_PLANNING, days: 2 },
          products: [
            { ...product, sku: 'COFFEE-250G' },
            { ...product, sku: 'MATCHA-50G' },
          ],
        },
        (message) => messages.push(message),
      );
      assert.equal(run.results.length, 2);
      assert.ok(
        run.results.every(
          (r) => r.inventory.length === 2 && r.reconciliation.purchasingConstraintsBalanced,
        ),
      );
      assert.ok(run.inputs[1].products.every((p) => p.leadTimeDays === 4));
      assert.ok(messages.some((message) => message.includes('Plan 2/2')));
      const compared = comparisonEvidence(run.results);
      assert.equal(compared.sameOpeningCash, true);
      assert.equal(compared.sameProductPriority, true);
      assert.equal(compared.sameDemandPaths, true);
      assert.equal(compared.sameSupplyPaths, false);
      const comparisonText = resultLines(
        { sim: { preset: 'Supplier delay', seed: 42, result: run } },
        160,
      ).join('\n');
      assert.match(comparisonText, /Projected paths: demand same · supply different/);
      const inputsText = comparisonText.split('EXECUTED ASSUMPTIONS')[1];
      assert.match(inputsText, /Baseline · 2 days/);
      assert.match(inputsText, /Supplier delay · 2 days/);
      assert.match(inputsText, /lead 1d/);
      assert.match(inputsText, /lead 4d/);
      // The UI's order is an economic input: only the first SKU can afford
      // replenishment when the shared cash pays for exactly one whole order.
      const priority = {
        ...navigate(initialState(), 5),
        panel: 'products',
        selected: 1,
        planning: { ...DEFAULT_PLANNING, days: 2, startingCash: '10.00' },
        products: ['COFFEE-250G', 'MATCHA-50G'].map((sku) => ({
          ...product,
          sku,
          initialStock: 0,
          unitPrice: '2.00',
          unitCost: '1.00',
          demand: { min: 0, max: 0 },
          reorderPoint: 0,
          targetStock: 10,
          purchasing: { minimumOrderQuantity: 1, orderMultiple: 1 },
        })),
      };
      const reordered = handleKey(priority, '[', { name: '[' });
      assert.equal(reordered.effect, undefined);
      for (const [state, expected] of [
        [priority, 'COFFEE-250G'],
        [reordered.state, 'MATCHA-50G'],
      ]) {
        const observed = await backend.request('simulate', {
          preset: 'baseline',
          seed: 42,
          controls: state.planning,
          products: state.products,
        });
        assert.equal(observed.inputs[0].products[0].sku, expected);
        const report = observed.results[0];
        const purchases = report.timeline.filter((event) => event.type === 'purchase_order_sent');
        assert.deepEqual(
          purchases.map((event) => event.sku),
          [expected],
        );
        assert.equal(report.summary.purchasingSpend, '10.00');
        assert.equal(report.summary.endingCash, '0.00');
        assert.equal(cashMovement(report), '-10.00');
        assert.equal(report.summary.grossMargin, '0.00');
        assert.equal(report.summary.endingInventoryValue, '10.00');
        assert.equal(report.summary.inTransitInventoryValue, '0.00');
        assert.equal(report.reconciliation.purchasingConstraintsBalanced, true);
        const explained = resultLines(
          {
            sim: { preset: 'Steady demand', seed: 42, controls: state.planning, result: observed },
          },
          160,
        ).join('\n');
        const refused = expected === 'COFFEE-250G' ? 'MATCHA-50G' : 'COFFEE-250G';
        assert.ok(explained.includes(`${refused}: 3 blocked attempt(s)`));
        assert.match(explained, /Lowest cash USD 0\.00 · day 0/);
        assert.match(explained, /Opening cash USD 10\.00/);
        assert.match(explained, /Cash movement USD -10\.00/);
        assert.match(explained, /Inventory at cost: on hand USD 10\.00 · in transit USD 0\.00/);
        assert.match(explained, /First refusal day 0: whole order USD 10\.00; cash USD 0\.00/);
        assert.ok(explained.includes(`Priority 1 · ${expected}`));
        const receipts = timelineEntries(report, `${expected} purchase_received`).entries;
        assert.equal(receipts.length, 1);
        assert.equal(receipts[0].event.day, 1);
        const refusals = timelineEntries(report, `${refused} budget`).entries;
        assert.deepEqual(
          refusals.map(({ event }) => event.day),
          [0, 1, 2],
        );
        const eventText = timelineLines(
          {
            panelQuery: `${expected} purchase_order_sent`,
            sim: { preset: 'Steady demand', seed: 42, result: observed },
          },
          160,
        ).join('\n');
        assert.ok(eventText.includes(`Day 0 · purchase order sent · ${expected}`));
        assert.match(eventText, /Purchase spend: USD 10\.00/);
        assert.match(eventText, /Cash after event: USD 0\.00/);
        assert.match(eventText, /Expected arrival day: 1/);
      }
      const reserved = await backend.request('simulate', {
        preset: 'cash',
        seed: 42,
        controls: { ...priority.planning, cashReserve: '0.01' },
        products: priority.products,
      });
      assert.deepEqual(
        reserved.inputs.map((plan) => plan.cashReserve),
        ['0.01', '0.01'],
      );
      assert.deepEqual(
        reserved.inputs.map((plan) => plan.startingCash),
        ['10.00', '0.50'],
      );
      assert.equal(reserved.results[0].summary.reserveBlockedPurchases, 6);
      assert.equal(reserved.results[0].summary.endingCash, '10.00');
      assert.equal(reserved.results[0].summary.purchasingSpend, '0.00');
      assert.equal(reserved.results[1].summary.reserveBlockedPurchases, 0);
      assert.equal(reserved.results[1].summary.endingCash, '0.50');
      assert.ok(
        reserved.results.every(
          (report) =>
            report.purchases.length === 0 &&
            report.reconciliation.purchasingConstraintsBalanced &&
            report.reconciliation.cashFlowBalanced,
        ),
      );
      const reserveText = resultLines(
        { sim: { preset: 'Cash constraint', seed: 42, result: reserved } },
        180,
      ).join('\n');
      assert.match(reserveText, /COFFEE-250G: 3 reserve-blocked attempt\(s\)/);
      assert.match(reserveText, /Reserved cash USD 0\.01; spendable cash USD 9\.99/);
      assert.match(reserveText, /Cash reserve USD 0\.01/);
      let started;
      const ready = new Promise((resolve) => {
        started = resolve;
      });
      const pending = backend.request(
        'simulate',
        { preset: 'delay', seed: 43, controls: DEFAULT_PLANNING },
        (message) => {
          if (message.startsWith('Plan 1/')) started();
        },
      );
      const rejected = assert.rejects(pending, /canceled/);
      pending.catch(() => started());
      await ready;
      assert.equal(backend.cancel('simulate'), true);
      await rejected;
      assert.equal(backend.cancel('simulate'), false);
      assert.ok(
        !(await fs.readdir(await backend.directory)).some((name) => name.startsWith('run-')),
        'canceled run directories are cleaned',
      );
      assert.equal(
        createHash('sha256')
          .update(await fs.readFile(source))
          .digest('hex'),
        before,
        'browsing and scenarios did not mutate source records',
      );
    } finally {
      await backend.close();
    }
  },
);

test(
  'native planning decisions explain capacity, reserve and off-cycle reviews without spending cash',
  { timeout: 45000 },
  async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-tui-decisions-'));
    try {
      const product = {
        initialStock: 0,
        unitPrice: '25.00',
        unitCost: '1.00',
        demand: { min: 0, max: 0 },
        reorderPoint: 0,
        targetStock: 10,
        leadTimeDays: 1,
      };
      const plan = {
        name: 'Purchasing rules',
        currency: 'USD',
        days: 2,
        seed: 42,
        startingCash: '10.00',
        cashReserve: '5.00',
        products: [
          {
            ...product,
            sku: 'CAPACITY',
            targetStock: 3,
            purchasing: { minimumOrderQuantity: 5, orderMultiple: 4, maximumInventoryPosition: 6 },
          },
          { ...product, sku: 'RESERVE', purchasing: { reviewIntervalDays: 3 } },
          { ...product, sku: 'BUDGET', targetStock: 1, unitCost: '20.00' },
        ],
      };
      const report = await executeInventoryPlan(plan, path.join(directory, 'private.db'));
      assert.equal(report.summary.purchasingSpend, '0.00');
      assert.equal(report.summary.endingCash, '10.00');
      assert.equal(report.summary.constraintBlockedPurchases, 3);
      assert.equal(report.summary.reserveBlockedPurchases, 1);
      assert.equal(report.summary.deferredPurchaseReviews, 2);
      assert.deepEqual(report.purchases, []);
      assert.equal(report.reconciliation.purchaseOrdersBalanced, true);
      assert.equal(report.reconciliation.cashFlowBalanced, true);
      assert.equal(report.reconciliation.inventoryFlowBalanced, true);
      const before = structuredClone(report);
      const text = resultLines(
        {
          sim: { preset: 'Native policy', seed: 42, result: { inputs: [plan], results: [report] } },
        },
        180,
      ).join('\n');
      assert.match(text, /CAPACITY: 3 capacity-blocked attempt\(s\)/);
      assert.match(text, /Inventory position with proposed order 8; capacity limit 6/);
      assert.match(text, /Target shortfall 3; rounded order 8; extra units 5/);
      assert.match(text, /RESERVE: 1 reserve-blocked attempt\(s\)/);
      assert.match(text, /Reserved cash USD 5\.00; spendable cash USD 5\.00/);
      assert.match(text, /RESERVE: 2 deferred review attempt\(s\)/);
      assert.match(text, /Review interval 3 day\(s\); next review day 3/);
      assert.match(text, /Next review is outside this report's horizon/);
      assert.match(text, /BUDGET: 3 blocked attempt\(s\)/);
      assert.deepEqual(report, before);
    } finally {
      await fs.rm(directory, { recursive: true, force: true });
    }
  },
);
