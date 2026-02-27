import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const MAIN_BIN = join(__dirname, '..', 'bin', 'stateset.js');

const tempDirs = new Set();

function runMain(args = [], env = {}) {
  return runNodeScript(MAIN_BIN, args, { env });
}

function createBatchFile(content = 'list customers\n') {
  const dir = mkdtempSync(join(tmpdir(), 'stateset-batch-'));
  tempDirs.add(dir);
  const file = join(dir, 'requests.txt');
  writeFileSync(file, content);
  return file;
}

afterEach(() => {
  for (const dir of tempDirs) {
    rmSync(dir, { recursive: true, force: true });
  }
  tempDirs.clear();
});

describe('stateset lifecycle and guardrails', () => {
  it('routes `stateset doctor --help` to stateset-doctor', () => {
    const result = runMain(['doctor', '--help']);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Health Check & Diagnostics/i);
  });

  it('routes `stateset update --help` to stateset-update', () => {
    const result = runMain(['update', '--help']);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Update Manager/i);
  });

  it('supports `--update` shorthand routing', () => {
    const result = runMain(['--update', '--help']);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Update Manager/i);
  });

  it('enforces --parallel cap', () => {
    const batchFile = createBatchFile();
    const result = runMain(['--batch', batchFile, '--parallel', '999']);
    assert.equal(result.status, 1);
    assert.match(`${result.stdout}${result.stderr}`, /cannot exceed/i);
  });

  it('requires queue admin guard for queue operations', () => {
    const result = runMain(['--queue-status', '--json']);
    assert.equal(result.status, 1);
    assert.match(`${result.stdout}${result.stderr}`, /queue admin commands require/i);
  });

  it('allows queue operations when queue admin guard is satisfied', () => {
    const result = runMain(
      ['--queue-status', '--queue-admin', '--json'],
      { STATESET_QUEUE_ADMIN: '1' },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.doesNotThrow(() => JSON.parse(result.stdout));
  });

  it('returns JSON validation errors when --json is set (provider)', () => {
    const result = runMain(['--json', '--provider', 'bogus', 'ping']);
    assert.equal(result.status, 1);
    assert.doesNotThrow(() => JSON.parse(result.stdout));
    const payload = JSON.parse(result.stdout);
    assert.match(payload.error, /unknown provider/i);
  });

  it('returns JSON validation errors when --json is set (timeout)', () => {
    const result = runMain(['--json', '--timeout', '0', 'ping']);
    assert.equal(result.status, 1);
    assert.doesNotThrow(() => JSON.parse(result.stdout));
    const payload = JSON.parse(result.stdout);
    assert.match(payload.error, /timeout.*positive integer/i);
  });
});
