import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const BIN_DIR = join(__dirname, '..', 'bin');

function runCli(bin, args = []) {
  return runNodeScript(join(BIN_DIR, bin), args);
}

const TARGETS = [
  'stateset-analytics.js',
  'stateset-chat.js',
  'stateset-checkout.js',
  'stateset-currency.js',
  'stateset-import.js',
  'stateset-inventory.js',
  'stateset-invoices.js',
  'stateset-manufacturing.js',
  'stateset-orders.js',
  'stateset-payments.js',
  'stateset-promotions.js',
  'stateset-returns.js',
  'stateset-shipments.js',
  'stateset-subscriptions.js',
  'stateset-suppliers.js',
  'stateset-tax.js',
  'stateset-warranties.js',
];

describe('--no-memory flag wiring', () => {
  for (const bin of TARGETS) {
    it(`${bin} accepts --no-memory`, () => {
      const result = runCli(bin, ['--no-memory', '--help']);
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assert.match(result.stdout, /\bUSAGE\b|Usage:/i);
    });
  }
});
