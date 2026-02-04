/**
 * CLI JSON output tests for stateset-autonomous
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
const CLI_PATH = join(__dirname, '..', 'bin', 'stateset-autonomous.js');

function runCli(args, opts = {}) {
  const result = spawnSync('node', [CLI_PATH, ...args], {
    encoding: 'utf-8',
    ...opts,
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

describe('stateset-autonomous CLI JSON output', () => {
  let workspace;
  let embeddedAvailable = true;

  before(async () => {
    workspace = mkdtempSync(join(tmpdir(), 'stateset-autonomous-cli-'));
    try {
      await import('@stateset/embedded');
    } catch (error) {
      embeddedAvailable = false;
    }
  });

  after(() => {
    if (workspace) {
      rmSync(workspace, { recursive: true, force: true });
    }
  });

  it('init, status, and jobs emit valid JSON', (t) => {
    if (!embeddedAvailable) {
      t.skip('Skipping: @stateset/embedded not available in this environment.');
      return;
    }

    const dbPath = join(workspace, 'commerce.db');
    const storePath = join(workspace, 'autonomous');

    const init = runCli(['init', '--db', dbPath, '--store', storePath, '--json']);
    assert.equal(init.status, 0, init.stderr);

    const initPayload = parseJson(init.stdout.trim());
    assert.equal(initPayload.success, true);
    assert.ok(initPayload.counts.jobs > 0);
    assert.ok(initPayload.counts.workflows > 0);
    assert.ok(initPayload.counts.policies > 0);

    const status = runCli(['status', '--db', dbPath, '--store', storePath, '--json']);
    assert.equal(status.status, 0, status.stderr);

    const statusPayload = parseJson(status.stdout.trim());
    assert.ok(statusPayload.status);
    assert.ok(statusPayload.status.features);

    const jobs = runCli(['jobs', '--db', dbPath, '--store', storePath, '--json']);
    assert.equal(jobs.status, 0, jobs.stderr);

    const jobsPayload = parseJson(jobs.stdout.trim());
    assert.ok(Array.isArray(jobsPayload.jobs));
    assert.ok(jobsPayload.total >= 0);

    const jobsList = runCli(['jobs', 'list', '--db', dbPath, '--store', storePath, '--json']);
    assert.equal(jobsList.status, 0, jobsList.stderr);

    const jobsListPayload = parseJson(jobsList.stdout.trim());
    assert.ok(Array.isArray(jobsListPayload.jobs));
    assert.ok(jobsListPayload.total >= 0);

    if (jobsPayload.jobs.length > 0) {
      const jobId = jobsPayload.jobs[0].id;
      const enable = runCli(['jobs', '--db', dbPath, '--store', storePath, '--enable', jobId, '--json']);
      assert.equal(enable.status, 0, enable.stderr);
      const enablePayload = parseJson(enable.stdout.trim());
      assert.equal(enablePayload.action, 'enable');
      assert.equal(enablePayload.job.id, jobId);
      assert.equal(enablePayload.job.enabled, true);

      const enableCmd = runCli(['jobs', 'enable', jobId, '--db', dbPath, '--store', storePath, '--json']);
      assert.equal(enableCmd.status, 0, enableCmd.stderr);
      const enableCmdPayload = parseJson(enableCmd.stdout.trim());
      assert.equal(enableCmdPayload.action, 'enable');
      assert.equal(enableCmdPayload.job.id, jobId);
      assert.equal(enableCmdPayload.job.enabled, true);

      const enabledList = runCli(['jobs', 'list', '--enabled', '--db', dbPath, '--store', storePath, '--json']);
      assert.equal(enabledList.status, 0, enabledList.stderr);
      const enabledListPayload = parseJson(enabledList.stdout.trim());
      assert.ok(enabledListPayload.jobs.some((job) => job.id === jobId));

      const disableCmd = runCli(['jobs', 'disable', jobId, '--db', dbPath, '--store', storePath, '--json']);
      assert.equal(disableCmd.status, 0, disableCmd.stderr);
      const disableCmdPayload = parseJson(disableCmd.stdout.trim());
      assert.equal(disableCmdPayload.action, 'disable');
      assert.equal(disableCmdPayload.job.id, jobId);
      assert.equal(disableCmdPayload.job.enabled, false);

      const disabledList = runCli(['jobs', 'list', '--disabled', '--db', dbPath, '--store', storePath, '--json']);
      assert.equal(disabledList.status, 0, disabledList.stderr);
      const disabledListPayload = parseJson(disabledList.stdout.trim());
      assert.ok(disabledListPayload.jobs.some((job) => job.id === jobId));

      const runCmd = runCli(['jobs', 'run', jobId, '--db', dbPath, '--store', storePath, '--json']);
      assert.equal(runCmd.status, 0, runCmd.stderr);
      const runCmdPayload = parseJson(runCmd.stdout.trim());
      assert.equal(runCmdPayload.action, 'run');
      assert.equal(runCmdPayload.jobId, jobId);
    }
  });
});
