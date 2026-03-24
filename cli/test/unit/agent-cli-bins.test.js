import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const exec = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN_DIR = path.join(__dirname, '..', '..', 'bin');

async function runCli(binName, args) {
  const cliPath = path.join(BIN_DIR, binName);

  try {
    const { stdout, stderr } = await exec(process.execPath, [cliPath, ...args], {
      timeout: 15000,
      env: { ...process.env, NODE_NO_WARNINGS: '1' },
    });
    return { stdout, stderr, exitCode: 0 };
  } catch (error) {
    return {
      stdout: error.stdout || '',
      stderr: error.stderr || '',
      exitCode: error.code ?? 1,
    };
  }
}

describe('commerce agent CLI bins', () => {
  it('shows stats-aware help for write-capable agent bins', async () => {
    const { stdout, exitCode } = await runCli('stateset-orders.js', ['--help']);

    assert.equal(exitCode, 0);
    assert.ok(stdout.includes('stateset-orders'));
    assert.ok(stdout.includes('--apply'));
    assert.ok(stdout.includes('--stats'));
  });

  it('shows stats-aware help for read-only analytics', async () => {
    const { stdout, exitCode } = await runCli('stateset-analytics.js', ['--help']);

    assert.equal(exitCode, 0);
    assert.ok(stdout.includes('stateset-analytics'));
    assert.ok(stdout.includes('--stats'));
    assert.ok(!stdout.includes('  --apply            Enable write operations'));
  });

  it('prints version output for checkout', async () => {
    const { stdout, exitCode } = await runCli('stateset-checkout.js', ['--version']);

    assert.equal(exitCode, 0);
    assert.match(stdout.trim(), /^@stateset\/cli checkout-agent v\d+\.\d+\.\d+/);
  });
});
