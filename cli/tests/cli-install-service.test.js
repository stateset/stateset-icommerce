/**
 * CLI JSON output tests for stateset-install-service
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const CLI_PATH = join(__dirname, '..', 'bin', 'stateset-install-service.js');

function runCli(args) {
  const result = spawnSync(process.execPath, [CLI_PATH, ...args], {
    encoding: 'utf-8',
    env: { ...process.env, NODE_NO_WARNINGS: '1' },
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

describe('stateset-install-service CLI JSON output', () => {
  it('dry-run emits structured JSON on supported platforms', (t) => {
    const supported = ['linux', 'darwin'].includes(process.platform);
    if (!supported) {
      t.skip(`Skipping: unsupported platform ${process.platform}.`);
      return;
    }

    const result = runCli(['--dry-run', '--json']);
    assert.equal(result.status, 0, result.stderr);

    const payload = parseJson(result.stdout.trim());
    assert.equal(payload.ok, true);
    assert.equal(payload.dryRun, true);
    assert.equal(payload.mode, 'install');
    assert.ok(Array.isArray(payload.steps));
    assert.ok(payload.steps.length > 0);
  });
});
