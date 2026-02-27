/**
 * CLI JSON output tests for stateset-x402 and stateset-x402-mcp
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
const X402_PATH = join(__dirname, '..', 'bin', 'stateset-x402.js');
const X402_MCP_PATH = join(__dirname, '..', 'bin', 'stateset-x402-mcp.js');

function runCli(binPath, args, opts = {}) {
  const env = { ...process.env, NODE_NO_WARNINGS: '1', ...(opts.env || {}) };
  const result = spawnSync(process.execPath, [binPath, ...args], {
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

describe('stateset-x402 CLI JSON output', () => {
  let workspace;

  before(() => {
    workspace = mkdtempSync(join(tmpdir(), 'stateset-x402-cli-'));
  });

  after(() => {
    if (workspace) {
      rmSync(workspace, { recursive: true, force: true });
    }
  });

  it('init emits JSON payload', () => {
    const result = runCli(
      X402_PATH,
      [
        'init',
        '--sequencer-url', 'https://sequencer.example.com',
        '--tenant-id', 'tenant-123',
        '--store-id', 'store-456',
        '--agent-id', 'agent-789',
        '--config-dir', join(workspace, '.stateset'),
        '--json',
      ],
      { cwd: workspace }
    );

    assert.equal(result.status, 0, result.stderr);
    const payload = parseJson(result.stdout.trim());
    assert.equal(payload.success, true);
    assert.equal(payload.tenantId, 'tenant-123');
    assert.equal(payload.storeId, 'store-456');
    assert.equal(payload.agentId, 'agent-789');
    assert.ok(payload.payerAddress);
    assert.ok(payload.configPath);
  });

  it('x402-mcp version prints CLI version', async () => {
    const { CLI_VERSION } = await import('../src/config.js');
    const result = runCli(X402_MCP_PATH, ['--version']);
    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes(CLI_VERSION));
  });
});
