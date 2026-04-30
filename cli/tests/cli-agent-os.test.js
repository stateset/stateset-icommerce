import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const AGENT_BIN = join(__dirname, '..', 'bin', 'stateset-agent.js');

const tempDirs = new Set();

function tempDir() {
  const dir = mkdtempSync(join(tmpdir(), 'stateset-agent-cli-'));
  tempDirs.add(dir);
  return dir;
}

function runAgent(args = [], options = {}) {
  return runNodeScript(AGENT_BIN, args, options);
}

afterEach(() => {
  for (const dir of tempDirs) {
    rmSync(dir, { recursive: true, force: true });
  }
  tempDirs.clear();
});

describe('stateset-agent CLI', () => {
  it('bootstraps a workspace that reaches A readiness', () => {
    const workspace = tempDir();
    const settingsPath = join(workspace, '.stateset', 'settings.json');
    const env = {
      STATESET_SETTINGS: settingsPath,
      ANTHROPIC_API_KEY: 'test-key',
    };

    const setup = runAgent(['setup', 'Launch Ops', '--json'], { cwd: workspace, env });
    assert.equal(setup.status, 0, setup.stderr || setup.stdout);
    const setupPayload = JSON.parse(setup.stdout);
    assert.equal(setupPayload.settings.memory.enabled, true);
    assert.equal(setupPayload.settings.channels.default, 'webchat');
    assert.equal(existsSync(settingsPath), true);
    assert.equal(existsSync(join(workspace, '.stateset', 'channels.json')), true);
    assert.equal(
      existsSync(join(workspace, '.stateset', 'skills', 'commerce-runbook-launch-ops', 'SKILL.md')),
      true,
    );

    const channelConfig = JSON.parse(readFileSync(join(workspace, '.stateset', 'channels.json')));
    assert.equal(channelConfig.channels.webchat.enabled, true);

    const status = runAgent(['status', '--json'], { cwd: workspace, env });
    assert.equal(status.status, 0, status.stderr || status.stdout);
    const statusPayload = JSON.parse(status.stdout);
    assert.equal(statusPayload.readiness.grade, 'A+');
    assert.equal(statusPayload.readiness.score, 100);
    assert.equal(
      statusPayload.channels.find((channel) => channel.name === 'webchat')?.configured,
      true,
    );

    const context = runAgent(['context', '--json'], { cwd: workspace, env });
    assert.equal(context.status, 0, context.stderr || context.stdout);
    const contextPayload = JSON.parse(context.stdout);
    assert.equal(contextPayload.memory.enabled, true);
    assert.equal(
      contextPayload.warnings.some((warning) => /Memory is disabled/i.test(warning)),
      false,
    );
  });
});
