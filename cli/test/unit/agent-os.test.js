import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import {
  collectAgentOsStatus,
  createRunbookSkill,
  inspectAgentContext,
  listAgentSkills,
  saveOperationalMemory,
  searchOperationalMemory,
  setupAgentWorkspace,
} from '../../src/agent-os.js';

const tempDirs = new Set();

function tempDir(prefix = 'stateset-agent-os-') {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tempDirs.add(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs) {
    rmSync(dir, { recursive: true, force: true });
  }
  tempDirs.clear();
});

describe('agent OS status', () => {
  it('summarizes providers, skills, sessions, and next actions without creating stores', () => {
    const root = tempDir();
    const memoryDir = join(root, 'memory');
    const sessionDbPath = join(root, 'agent-sessions.db');
    const workspaceSkillDir = join(root, 'skills');

    const status = collectAgentOsStatus({
      env: { ANTHROPIC_API_KEY: 'test-key' },
      memoryDir,
      sessionDbPath,
      workspaceSkillDir,
      limit: 3,
    });

    assert.equal(status.version, '1.0.1');
    assert.equal(status.providers.find((provider) => provider.id === 'claude')?.configured, true);
    assert.ok(status.skills.total > 0, 'expected bundled commerce skills to be discovered');
    assert.equal(status.sessions.count, 0);
    assert.equal(status.memory.exists, false);
    assert.ok(Array.isArray(status.nextActions));
  });
});

describe('agent OS runbooks', () => {
  it('creates workspace runbook skills and refuses accidental overwrite', async () => {
    const workspaceDir = join(tempDir(), 'skills');

    const result = await createRunbookSkill({
      name: 'Daily Ops',
      description: 'daily commerce operations',
      workspaceDir,
    });

    assert.equal(result.name, 'commerce-runbook-daily-ops');
    const content = readFileSync(result.path, 'utf-8');
    assert.match(content, /name: commerce-runbook-daily-ops/);
    assert.match(content, /## Procedure/);

    await assert.rejects(
      createRunbookSkill({ name: 'Daily Ops', workspaceDir }),
      /already exists/i,
    );

    const skills = listAgentSkills({
      origin: 'workspace',
      workspaceSkillDir: workspaceDir,
    });
    assert.equal(skills.some((skill) => skill.name === 'commerce-runbook-daily-ops'), true);
  });
});

describe('agent OS workspace setup', () => {
  it('bootstraps workspace settings, memory dirs, and a launch runbook', async () => {
    const root = tempDir();
    const result = await setupAgentWorkspace({
      cwd: root,
      settingsPath: join(root, '.stateset', 'settings.json'),
      workspaceSkillDir: join(root, '.stateset', 'skills'),
      memoryDir: join(root, '.stateset', 'memory'),
      memoryDbPath: join(root, '.stateset', 'memory.db'),
      sessionDbPath: join(root, '.stateset', 'agent-sessions.db'),
      provider: 'openai',
      agent: 'inventory',
    });

    const settings = JSON.parse(readFileSync(result.settingsPath, 'utf-8'));
    assert.equal(settings.provider.default, 'openai');
    assert.equal(settings.agent.default, 'inventory');
    assert.equal(settings.memory.enabled, true);
    assert.equal(settings.memory.useMarkdown, true);
    assert.equal(settings.memory.dir, '.stateset/memory');
    assert.equal(settings.sessionStore.dbPath, '.stateset/agent-sessions.db');
    assert.equal(settings.channels.configPath, '.stateset/channels.json');
    assert.equal(settings.channels.default, 'webchat');
    assert.equal(settings.contextGuard.enabled, true);
    assert.equal(settings.privacy.redactMemory, true);
    assert.equal(existsSync(join(root, '.stateset', 'memory', 'sessions')), true);
    assert.equal(existsSync(join(root, '.stateset', 'channels.json')), true);
    assert.equal(existsSync(result.runbook.path), true);

    const channelConfig = JSON.parse(readFileSync(result.channelConfigPath, 'utf-8'));
    assert.equal(channelConfig.channels.webchat.enabled, true);

    const status = collectAgentOsStatus({
      env: { ANTHROPIC_API_KEY: 'test-key' },
      cwd: root,
      settings,
      memoryDir: join(root, '.stateset', 'memory'),
      sessionDbPath: join(root, '.stateset', 'agent-sessions.db'),
      workspaceSkillDir: join(root, '.stateset', 'skills'),
    });
    assert.equal(status.channels.find((channel) => channel.name === 'webchat')?.configured, true);
    assert.equal(status.readiness.grade, 'A+');
  });

  it('preserves existing provider and custom settings unless explicitly overridden', async () => {
    const root = tempDir();
    const settingsPath = join(root, '.stateset', 'settings.json');
    mkdirSync(join(root, '.stateset'), { recursive: true });
    writeFileSync(
      settingsPath,
      JSON.stringify(
        {
          provider: { default: 'gemini' },
          memory: { maxSummaries: 9 },
          custom: { launchTier: 'enterprise' },
        },
        null,
        2,
      ),
    );

    const result = await setupAgentWorkspace({
      cwd: root,
      settingsPath,
      workspaceSkillDir: join(root, '.stateset', 'skills'),
      memoryDir: join(root, '.stateset', 'memory'),
    });
    const settings = result.settings;

    assert.equal(settings.provider.default, 'gemini');
    assert.equal(settings.memory.maxSummaries, 9);
    assert.equal(settings.memory.enabled, true);
    assert.equal(settings.custom.launchTier, 'enterprise');
  });
});

describe('agent OS context inspection', () => {
  it('surfaces token-heavy sessions and compactions', () => {
    const root = tempDir();
    const sessionDbPath = join(root, 'agent-sessions.db');
    writeFileSync(
      `${sessionDbPath}.fallback.json`,
      JSON.stringify({
        rows: [
          {
            session_id: 'sess-heavy',
            provider: 'claude',
            model: 'claude-test',
            agent: 'customer-service',
            total_tokens: 125000,
            compaction_count: 2,
            updated_at: Date.now(),
            created_at: Date.now(),
          },
        ],
      }),
    );

    const context = inspectAgentContext({
      env: { ANTHROPIC_API_KEY: 'test-key' },
      memoryDir: join(root, 'memory'),
      sessionDbPath,
      workspaceSkillDir: join(root, 'skills'),
      limit: 5,
    });

    assert.equal(context.sessions.highTokenSessions[0].sessionId, 'sess-heavy');
    assert.equal(context.sessions.totalCompactions, 2);
    assert.ok(context.warnings.some((warning) => /token-heavy/i.test(warning)));
  });
});

describe('agent OS operational memory', () => {
  it('saves and searches markdown operational memory', async () => {
    const memoryDir = join(tempDir(), 'memory');

    await saveOperationalMemory({
      memoryDir,
      summary: 'Warehouse cutoff moved to 3pm for launch week',
      facts: ['Notify customer-service before promising same-day fulfillment'],
      agent: 'operations',
      sessionId: 'session-1',
    });

    const matches = await searchOperationalMemory({
      memoryDir,
      query: 'cutoff',
      limit: 5,
    });

    assert.equal(matches.length, 1);
    assert.match(matches[0].summary, /Warehouse cutoff/);
    assert.deepEqual(matches[0].facts, [
      'Notify customer-service before promising same-day fulfillment',
    ]);
  });
});
