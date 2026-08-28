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
