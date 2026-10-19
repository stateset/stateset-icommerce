import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import Database from 'better-sqlite3';
import { TerminalBackend } from '../../src/tui/backend.js';
import { DEFAULT_PLANNING } from '../../src/tui/planning.js';

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
