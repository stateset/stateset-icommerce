import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const BIN_DIR = join(__dirname, '..', 'bin');

function runCli(bin, args = []) {
  const result = spawnSync('node', [join(BIN_DIR, bin), ...args], {
    encoding: 'utf-8',
  });

  if (result.error) {
    throw result.error;
  }

  return result;
}

const HELP_TARGETS = [
  { bin: 'stateset.js', label: 'stateset' },
  { bin: 'stateset-orders.js', label: 'stateset-orders' },
  { bin: 'stateset-inventory.js', label: 'stateset-inventory' },
  { bin: 'stateset-returns.js', label: 'stateset-returns' },
  { bin: 'stateset-payments.js', label: 'stateset-payments' },
  { bin: 'stateset-promotions.js', label: 'stateset-promotions' },
  { bin: 'stateset-config.js', label: 'stateset-config' },
  { bin: 'stateset-doctor.js', label: 'stateset-doctor' },
  { bin: 'stateset-mcp-events.js', label: 'stateset-mcp-events' },
];

const VERSION_TARGETS = [
  { bin: 'stateset.js', label: 'stateset' },
  { bin: 'stateset-orders.js', label: 'stateset-orders' },
  { bin: 'stateset-inventory.js', label: 'stateset-inventory' },
  { bin: 'stateset-returns.js', label: 'stateset-returns' },
  { bin: 'stateset-mcp-events.js', label: 'stateset-mcp-events' },
];

const REQUEST_REQUIRED_TARGETS = [
  { bin: 'stateset.js', label: 'stateset' },
  { bin: 'stateset-orders.js', label: 'stateset-orders' },
  { bin: 'stateset-inventory.js', label: 'stateset-inventory' },
  { bin: 'stateset-returns.js', label: 'stateset-returns' },
  { bin: 'stateset-payments.js', label: 'stateset-payments' },
];

function assertHelpOutput(output, label) {
  const text = output.trim();
  assert.ok(text.length > 0, `${label} --help returned empty output`);
  assert.ok(/\bUSAGE\b|Usage:/i.test(text), `${label} --help missing usage section`);
}

describe('CLI help output', () => {
  for (const target of HELP_TARGETS) {
    it(`${target.label} --help exits 0 with usage`, () => {
      const result = runCli(target.bin, ['--help']);
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assertHelpOutput(result.stdout, target.label);
    });
  }
});

describe('CLI version output', () => {
  for (const target of VERSION_TARGETS) {
    it(`${target.label} --version exits 0 with version`, () => {
      const result = runCli(target.bin, ['--version']);
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assert.ok(result.stdout.trim().length > 0, `${target.label} --version returned empty output`);
    });
  }
});

describe('CLI missing request handling', () => {
  for (const target of REQUEST_REQUIRED_TARGETS) {
    it(`${target.label} exits 1 without request`, () => {
      const result = runCli(target.bin, []);
      assert.equal(result.status, 1);

      const output = `${result.stdout}${result.stderr}`;
      assert.ok(/usage:/i.test(output), `${target.label} missing usage hint`);
    });
  }
});
