import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const exec = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SIM_CLI_PATH = path.join(__dirname, '..', '..', 'bin', 'stateset-simulate.js');
const MAIN_CLI_PATH = path.join(__dirname, '..', '..', 'bin', 'stateset.js');

async function runCli(scriptPath, args, opts = {}) {
  try {
    const { stdout, stderr } = await exec(process.execPath, [scriptPath, ...args], {
      timeout: 20000,
      env: { ...process.env, NODE_NO_WARNINGS: '1' },
      ...opts,
    });
    return { stdout, stderr, exitCode: 0 };
  } catch (err) {
    return {
      stdout: err.stdout || '',
      stderr: err.stderr || '',
      exitCode: err.code ?? 1,
    };
  }
}

describe('stateset-simulate CLI', () => {
  it('shows help text', async () => {
    const { stdout, exitCode } = await runCli(SIM_CLI_PATH, ['--help']);
    assert.equal(exitCode, 0);
    assert.ok(stdout.includes('stateset-simulate'));
    assert.ok(stdout.includes('supplier-goes-offline'));
  });

  it('lists built-in scenarios', async () => {
    const { stdout, exitCode } = await runCli(SIM_CLI_PATH, ['--list-scenarios', '--json']);
    assert.equal(exitCode, 0);
    const result = JSON.parse(stdout);
    assert.ok(Array.isArray(result.scenarios));
    assert.ok(result.scenarios.includes('supplier-goes-offline'));
  });

  it('runs supplier-goes-offline as JSON', async () => {
    const { stdout, exitCode } = await runCli(SIM_CLI_PATH, [
      '--scenario',
      'supplier-goes-offline',
      '--agents',
      'inventory,procurement',
      '--json',
    ]);
    assert.equal(exitCode, 0, stdout);
    const result = JSON.parse(stdout);
    assert.equal(result.success, true);
    assert.equal(result.outcome.finalRfqStatus, 'expired');
    assert.equal(result.outcome.fallbackRecommended, true);
  });

  it('routes from `stateset simulate --help` through the main CLI', async () => {
    const { stdout, exitCode } = await runCli(MAIN_CLI_PATH, ['simulate', '--help']);
    assert.equal(exitCode, 0);
    assert.ok(stdout.includes('StateSet Simulation CLI'));
    assert.ok(stdout.includes('stateset simulate --scenario'));
  });
});
