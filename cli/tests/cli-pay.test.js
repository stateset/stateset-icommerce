/**
 * CLI JSON output tests for stateset-pay
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const CLI_PATH = join(__dirname, '..', 'bin', 'stateset-pay.js');

function runCli(args, opts = {}) {
  const env = { ...process.env, NODE_NO_WARNINGS: '1', ...(opts.env || {}) };
  const result = spawnSync(process.execPath, [CLI_PATH, ...args], {
    encoding: 'utf-8',
    ...opts,
    env,
  });

  if (result.error) {
    throw result.error;
  }

  return result;
}

function parseJson(output) {
  try {
    return JSON.parse(output);
  } catch (error) {
    throw new Error(`Failed to parse JSON output: ${error.message}\nOutput:\n${output}`);
  }
}

describe('stateset-pay CLI JSON output', () => {
  let workspace;

  before(() => {
    workspace = mkdtempSync(join(tmpdir(), 'stateset-pay-cli-'));
  });

  after(() => {
    if (workspace) {
      rmSync(workspace, { recursive: true, force: true });
    }
  });

  it('chains command emits JSON array', () => {
    const result = runCli(['--chains', '--json']);
    assert.equal(result.status, 0, result.stderr);

    const payload = parseJson(result.stdout.trim());
    assert.ok(Array.isArray(payload));
    assert.ok(payload.length > 0);
    assert.ok(payload.some((c) => c.id === 'solana'));
  });

  it('wallet command emits JSON payload', () => {
    const result = runCli(
      ['--wallet', '--chain', 'solana', '--agent', 'test-agent', '--json'],
      { cwd: workspace }
    );
    assert.equal(result.status, 0, result.stderr);

    const payload = parseJson(result.stdout.trim());
    assert.equal(payload.agent, 'test-agent');
    assert.equal(payload.chain, 'solana');
    assert.ok(typeof payload.address === 'string' && payload.address.length > 0);
    assert.ok(typeof payload.explorerUrl === 'string' && payload.explorerUrl.length > 0);
  });
});
