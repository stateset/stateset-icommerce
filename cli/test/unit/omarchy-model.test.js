import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import { describe, it } from 'node:test';
import * as path from 'node:path';
import { runInNewContext } from 'node:vm';
import { fileURLToPath } from 'node:url';

const moduleValue = { exports: {} };
const modelFile = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../omarchy/Model.js',
);
runInNewContext(fs.readFileSync(modelFile, 'utf8'), { module: moduleValue, isFinite, parseInt });
const Model = moduleValue.exports;

describe('Omarchy shell model', () => {
  it('normalizes malformed alert values', () => {
    const normalized = JSON.parse(
      JSON.stringify(Model.normalizeAlerts({ pendingOrders: '2', failedPayments: -3 })),
    );
    assert.deepEqual(normalized, {
      pendingOrders: 2,
      failedPayments: 0,
      pendingReturns: 0,
      lowStock: 0,
      total: 2,
    });
  });

  it('normalizes status into a bounded fixed-field schema', () => {
    const normalized = JSON.parse(
      JSON.stringify(
        Model.normalizeStatus({
          ok: true,
          configured: true,
          dbPath: '/stores/<b>shop</b>.db',
          mode: 'unexpected-mode',
          message: '<img src=x onerror=alert(1)>\u202e ready',
          counts: { orders: '12x', customers: 4.9, products: 2_000_000_000 },
          alerts: { failedPayments: [99], lowStock: '3' },
          ignored: new Array(1000).fill('untrusted'),
        }),
      ),
    );

    assert.deepEqual(normalized, {
      ok: true,
      configured: true,
      dbPath: '/stores/‹b›shop‹/b›.db',
      mode: 'preview',
      message: '‹img src=x onerror=alert(1)› ready',
      counts: { orders: 0, customers: 4, products: 999999999, returns: 0, payments: 0 },
      alerts: {
        pendingOrders: 0,
        failedPayments: 0,
        pendingReturns: 0,
        lowStock: 3,
        total: 3,
      },
    });
  });

  it('rejects oversized, deeply nested, and excessive array-bearing status JSON', () => {
    assert.throws(() => Model.parseStatusJson('x'.repeat(Model.MAX_OUTPUT_CHARS + 1)), /size/);
    assert.doesNotThrow(() => Model.parseStatusJson('{"counts":[],"attention":[]}'));
    assert.throws(
      () => Model.parseStatusJson(`{"attention":[${new Array(130).fill('0').join(',')}]}`),
      /too many values/,
    );
    assert.throws(
      () => Model.parseStatusJson('{"a":{"b":{"c":{"d":{"e":{"f":{"g":{"h":{"i":1}}}}}}}}}'),
      /deeply nested/,
    );
    assert.throws(() => Model.parseStatusJson('{"ok":true'), /malformed/);
  });

  it('caps streamed output without retaining overflow', () => {
    const first = Model.appendBounded('', '1234', 5);
    const second = Model.appendBounded(first.text, '56789', 5);
    assert.deepEqual(JSON.parse(JSON.stringify(first)), { text: '1234', truncated: false });
    assert.deepEqual(JSON.parse(JSON.stringify(second)), { text: '12345', truncated: true });
  });

  it('builds a compact attention summary', () => {
    assert.equal(
      Model.attentionSummary({ failedPayments: 1, lowStock: 2, pendingReturns: 1 }),
      '1 failed payment · 2 low-stock SKUs · 1 pending return',
    );
  });

  it('notifies only when actionable alerts increase after the baseline', () => {
    const before = { failedPayments: 1, lowStock: 2, pendingReturns: 0 };
    assert.equal(Model.shouldNotify({}, before, false), false);
    assert.equal(Model.shouldNotify(before, { ...before, pendingOrders: 3 }, true), false);
    assert.equal(Model.shouldNotify(before, { ...before, lowStock: 3 }, true), true);
  });
});
