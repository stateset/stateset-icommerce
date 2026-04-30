import { createRequire } from 'node:module';
import fs from 'node:fs';
import { promises as fsp } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import memoryModule, { MarkdownMemoryStore } from './memory/markdown-store.js';
import { discoverSkills } from './skills/loader.js';
import { SkillRegistry } from './skills/registry.js';
import { AGENTS } from './agent-definitions.js';
import { PROVIDERS, DEFAULT_MODEL } from './config.js';
import { loadAgentSettings } from './settings.js';
import { DEFAULT_DB_PATH as DEFAULT_SESSION_DB_PATH } from './agent-session-store.js';

const require = createRequire(import.meta.url);
const { parseMemoryFile } = memoryModule;

export const DEFAULT_AGENT_MEMORY_DIR = path.join(os.homedir(), '.stateset', 'memory');
export const DEFAULT_WORKSPACE_SKILLS_DIR = path.resolve('.stateset', 'skills');
export const DEFAULT_WORKSPACE_AGENT_DIR = path.resolve('.stateset');
const FALLBACK_SESSION_SUFFIX = '.fallback.json';
const RUNBOOK_PREFIX = 'commerce-runbook';
const CHANNELS = [
  { name: 'webchat', bin: 'stateset-channels', env: ['STATESET_CHANNELS_CONFIG'] },
  {
    name: 'slack',
    bin: 'stateset-slack',
    env: ['SLACK_BOT_TOKEN', 'SLACK_APP_TOKEN', 'STATESET_SLACK_SIGNING_SECRET'],
  },
  { name: 'discord', bin: 'stateset-discord', env: ['DISCORD_TOKEN'] },
  { name: 'telegram', bin: 'stateset-telegram', env: ['TELEGRAM_BOT_TOKEN'] },
  { name: 'whatsapp', bin: 'stateset-whatsapp', env: ['WHATSAPP_SESSION_DIR'] },
  { name: 'signal', bin: 'stateset-signal', env: ['SIGNAL_PHONE_NUMBER'] },
  { name: 'google-chat', bin: 'stateset-google-chat', env: ['GOOGLE_APPLICATION_CREDENTIALS'] },
  { name: 'channels', bin: 'stateset-channels', env: ['STATESET_CHANNELS_CONFIG'] },
  { name: 'daemon', bin: 'stateset-daemon', env: ['STATESET_DAEMON_PORT'] },
];

function normalizeLimit(value, fallback = 5) {
  const numeric = Number.parseInt(String(value ?? ''), 10);
  return Number.isFinite(numeric) && numeric > 0 ? numeric : fallback;
}

function pathExists(filePath) {
  if (!filePath) return false;
  try {
    return fs.existsSync(filePath);
  } catch {
    return false;
  }
}

function safeReadDir(dirPath) {
  try {
    return fs.readdirSync(dirPath, { withFileTypes: true });
  } catch (error) {
    if (error?.code === 'ENOENT') return [];
    return [];
  }
}

function safeReadFile(filePath) {
  try {
    return fs.readFileSync(filePath, 'utf-8');
  } catch (error) {
    if (error?.code === 'ENOENT') return '';
    return '';
  }
}

function safeJsonParse(raw, fallback) {
  try {
    return JSON.parse(raw);
  } catch {
    return fallback;
  }
}

function slugify(value) {
  return String(value || '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

function truncate(value, width = 96) {
  const text = String(value || '');
  return text.length > width ? `${text.slice(0, width - 3)}...` : text;
}

function relativeIfInside(cwd, targetPath) {
  const absolute = path.resolve(targetPath);
  const relative = path.relative(path.resolve(cwd), absolute);
  if (!relative || relative.startsWith('..') || path.isAbsolute(relative)) {
    return absolute;
  }
  return relative;
}

function formatAge(timestamp) {
  if (!timestamp) return 'unknown';
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return 'unknown';
  return date.toISOString();
}

function providerReadiness(env = process.env) {
  return Object.entries(PROVIDERS).map(([id, provider]) => {
    const envKey = provider.envKey || null;
    const localConfigured = id === 'ollama' && Boolean(env.OLLAMA_HOST || env.STATESET_OLLAMA_HOST);
    return {
      id,
      name: provider.name,
      defaultModel: provider.default,
      envKey,
      configured: envKey ? Boolean(env[envKey]) : localConfigured,
      local: id === 'ollama',
    };
  });
}

function createRegistry({ workspaceSkillDir = DEFAULT_WORKSPACE_SKILLS_DIR } = {}) {
  const discovered = discoverSkills({ workspaceDir: workspaceSkillDir, verbose: false });
  const registry = new SkillRegistry();
  registry.loadFromDiscovered(discovered);
  return { discovered, registry };
}

function summarizeSkills(registry, limit = 5) {
  const stats = registry.getStats();
  const allSkills = registry.list();
  const top = allSkills.slice(0, limit).map((skill) => ({
    name: skill.name,
    category: skill.category,
    origin: skill.origin,
    description: skill.description,
  }));

  return {
    ...stats,
    runbooks: allSkills.filter((skill) => skill.name.includes('runbook')).length,
    top,
  };
}

function inspectMemory(memoryDir = DEFAULT_AGENT_MEMORY_DIR) {
  const mainPath = path.join(memoryDir, 'MEMORY.md');
  const mainContent = safeReadFile(mainPath);
  const mainEntries = mainContent ? parseMemoryFile(mainContent) : [];
  const sessions = safeReadDir(path.join(memoryDir, 'sessions')).filter(
    (entry) => entry.isFile() && entry.name.endsWith('.md'),
  );
  const entities = safeReadDir(path.join(memoryDir, 'entities')).filter(
    (entry) => entry.isFile() && entry.name.endsWith('.md'),
  );
  const topics = safeReadDir(path.join(memoryDir, 'topics')).filter(
    (entry) => entry.isFile() && entry.name.endsWith('.md'),
  );

  return {
    dir: memoryDir,
    exists: pathExists(memoryDir),
    mainPath,
    mainEntries: mainEntries.length,
    sessionFiles: sessions.length,
    entityFiles: entities.length,
    topicFiles: topics.length,
    latest: mainEntries.at(-1) || null,
  };
}

function normalizeSessionRow(row) {
  if (!row) return null;
  const summaries = Array.isArray(row.summaries)
    ? row.summaries
    : safeJsonParse(row.summaries || '[]', []);
  return {
    sessionId: row.session_id || row.sessionId,
    provider: row.provider || null,
    model: row.model || null,
    thinkLevel: row.think_level || row.thinkLevel || null,
    slaLevel: row.sla_level || row.slaLevel || null,
    agent: row.agent || null,
    summaries,
    lastRequest: row.last_request || row.lastRequest || null,
    lastResponse: row.last_response || row.lastResponse || null,
    lastError: row.last_error || row.lastError || null,
    lastErrorCode: row.last_error_code || row.lastErrorCode || null,
    abortedLastRun: Boolean(row.aborted_last_run || row.abortedLastRun),
    lastRunMs: row.last_run_ms ?? row.lastRunMs ?? null,
    lastCostUsd: row.last_cost_usd ?? row.lastCostUsd ?? null,
    totalCostUsd: row.total_cost_usd ?? row.totalCostUsd ?? null,
    totalTokens: row.total_tokens ?? row.totalTokens ?? null,
    compactionCount: row.compaction_count ?? row.compactionCount ?? 0,
    createdAt: row.created_at || row.createdAt || null,
    updatedAt: row.updated_at || row.updatedAt || null,
  };
}

function sortSessionsByRecency(rows) {
  return rows.sort((a, b) => {
    const bUpdated = Number(b.updatedAt || 0);
    const aUpdated = Number(a.updatedAt || 0);
    return bUpdated - aUpdated || String(a.sessionId).localeCompare(String(b.sessionId));
  });
}

function readFallbackSessions(dbPath, limit) {
  const fallbackPath = `${dbPath}${FALLBACK_SESSION_SUFFIX}`;
  if (!pathExists(fallbackPath)) {
    return null;
  }

  const payload = safeJsonParse(safeReadFile(fallbackPath), { rows: [] });
  const rows = sortSessionsByRecency((payload.rows || []).map(normalizeSessionRow).filter(Boolean));
  return {
    dbPath,
    backend: 'json-fallback',
    readable: true,
    count: rows.length,
    recent: rows.slice(0, limit),
    failures: rows
      .filter((row) => row.lastError || row.lastErrorCode || row.abortedLastRun)
      .slice(0, limit),
  };
}

function loadBetterSqlite() {
  try {
    const mod = require('better-sqlite3');
    return mod.default || mod;
  } catch {
    return null;
  }
}

function readSqliteSessions(dbPath, limit) {
  if (!pathExists(dbPath)) {
    return null;
  }

  const Database = loadBetterSqlite();
  if (!Database) {
    return {
      dbPath,
      backend: 'sqlite',
      readable: false,
      count: 0,
      recent: [],
      failures: [],
      reason: 'better-sqlite3 is unavailable in this install',
    };
  }

  let db = null;
  try {
    db = new Database(dbPath, { readonly: true, fileMustExist: true });
    const table = db
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'agent_sessions'")
      .get();
    if (!table) {
      return {
        dbPath,
        backend: 'sqlite',
        readable: true,
        count: 0,
        recent: [],
        failures: [],
      };
    }
    const count = db.prepare('SELECT COUNT(*) AS count FROM agent_sessions').get().count;
    const recent = db
      .prepare('SELECT * FROM agent_sessions ORDER BY updated_at DESC LIMIT ?')
      .all(limit)
      .map(normalizeSessionRow);
    const failures = db
      .prepare(
        `SELECT *
           FROM agent_sessions
          WHERE last_error IS NOT NULL OR last_error_code IS NOT NULL OR aborted_last_run = 1
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .all(limit)
      .map(normalizeSessionRow);
    return {
      dbPath,
      backend: 'sqlite',
      readable: true,
      count,
      recent,
      failures,
    };
  } catch (error) {
    return {
      dbPath,
      backend: 'sqlite',
      readable: false,
      count: 0,
      recent: [],
      failures: [],
      reason: error.message,
    };
  } finally {
    try {
      db?.close();
    } catch {
      // Ignore close failures from read-only status checks.
    }
  }
}

function inspectSessions(sessionDbPath = DEFAULT_SESSION_DB_PATH, limit = 5) {
  const safeLimit = normalizeLimit(limit);
  const fallback = readFallbackSessions(sessionDbPath, safeLimit);
  if (fallback) return fallback;

  const sqlite = readSqliteSessions(sessionDbPath, safeLimit);
  if (sqlite) return sqlite;

  return {
    dbPath: sessionDbPath,
    backend: null,
    readable: true,
    count: 0,
    recent: [],
    failures: [],
  };
}

function mergeSetupSettings(existing, setup, overrides = {}) {
  const merged = {
    ...existing,
    agent: { ...setup.agent, ...(existing.agent || {}) },
    provider: { ...setup.provider, ...(existing.provider || {}) },
    model: { ...setup.model, ...(existing.model || {}) },
    memory: { ...setup.memory, ...(existing.memory || {}) },
    sessionStore: { ...setup.sessionStore, ...(existing.sessionStore || {}) },
    channels: { ...(setup.channels || {}), ...(existing.channels || {}) },
    contextGuard: { ...setup.contextGuard, ...(existing.contextGuard || {}) },
    privacy: { ...setup.privacy, ...(existing.privacy || {}) },
  };

  merged.memory.enabled = true;
  merged.memory.useMarkdown = true;
  merged.sessionStore.enabled = true;
  merged.contextGuard.enabled = true;
  merged.privacy.redactLogs = true;
  merged.privacy.redactMemory = true;

  if (overrides.agent) {
    merged.agent.default = overrides.agent;
  }
  if (overrides.provider) {
    merged.provider.default = overrides.provider;
  }

  return merged;
}

function readJsonFile(filePath) {
  if (!pathExists(filePath)) return {};
  const raw = safeReadFile(filePath).trim();
  if (!raw) return {};
  return safeJsonParse(raw, {});
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function resolvePathFrom(cwd, maybePath) {
  if (!maybePath) return null;
  return path.isAbsolute(maybePath) ? maybePath : path.join(cwd, maybePath);
}

async function ensureMemoryDirs(memoryDir) {
  await fsp.mkdir(memoryDir, { recursive: true });
  await fsp.mkdir(path.join(memoryDir, 'sessions'), { recursive: true });
  await fsp.mkdir(path.join(memoryDir, 'entities'), { recursive: true });
  await fsp.mkdir(path.join(memoryDir, 'topics'), { recursive: true });
}

function buildChannelConfig({
  channel = 'webchat',
  dbPath = './store.db',
  agent = 'customer-service',
} = {}) {
  return {
    shared: {
      dbPath,
      allowApply: false,
      agent,
    },
    httpGateway: {
      enabled: true,
      host: '127.0.0.1',
      port: 0,
      allowAnonymous: false,
    },
    middleware: {
      logger: true,
      rateLimiter: {
        maxPerMinute: 20,
        maxPerHour: 200,
      },
    },
    channels: {
      [channel]: {
        enabled: true,
      },
    },
  };
}

function mergeChannelConfig(existing, setup) {
  return {
    ...setup,
    ...existing,
    shared: { ...setup.shared, ...(existing.shared || {}) },
    httpGateway: { ...setup.httpGateway, ...(existing.httpGateway || {}) },
    middleware: { ...setup.middleware, ...(existing.middleware || {}) },
    channels: { ...setup.channels, ...(existing.channels || {}) },
  };
}

function inspectChannelConfig({ env = process.env, settings = {}, cwd = process.cwd() } = {}) {
  const configPath = env.STATESET_CHANNELS_CONFIG || settings.channels?.configPath || null;
  const resolvedPath = resolvePathFrom(cwd, configPath);
  if (!resolvedPath || !pathExists(resolvedPath)) {
    return {
      path: resolvedPath,
      exists: false,
      enabledChannels: [],
    };
  }

  const parsed = readJsonFile(resolvedPath);
  const enabledChannels = Object.entries(parsed.channels || {})
    .filter(([, config]) => config?.enabled !== false)
    .map(([name]) => name);
  return {
    path: resolvedPath,
    exists: true,
    enabledChannels,
  };
}

function inspectChannels({ env = process.env, settings = {}, cwd = process.cwd() } = {}) {
  const config = inspectChannelConfig({ env, settings, cwd });
  return CHANNELS.map((channel) => {
    const configuredKeys = channel.env.filter((key) => Boolean(env[key]));
    const configuredByConfig =
      config.exists &&
      (config.enabledChannels.includes(channel.name) ||
        (channel.name === 'channels' && config.enabledChannels.length > 0));
    return {
      ...channel,
      available: true,
      configured: configuredKeys.length > 0 || configuredByConfig,
      configuredKeys,
      configPath: configuredByConfig ? config.path : null,
    };
  });
}

function buildReadiness({
  providers,
  settings,
  skills,
  memory,
  sessions,
  channels,
  workspaceSkillDir,
}) {
  const checks = [
    {
      id: 'provider',
      label: 'LLM provider configured',
      ok: providers.some((provider) => provider.configured),
    },
    {
      id: 'skills',
      label: 'Commerce skills loaded',
      ok: skills.total > 0,
    },
    {
      id: 'runbooks',
      label: 'Commerce runbooks available',
      ok: skills.runbooks > 0,
    },
    {
      id: 'memory',
      label: 'Operational memory enabled',
      ok: Boolean(settings.memory?.enabled),
    },
    {
      id: 'sessions',
      label: 'Session continuity available',
      ok: settings.sessionStore?.enabled !== false && sessions.readable,
    },
    {
      id: 'channels',
      label: 'At least one channel configured',
      ok: channels.some((channel) => channel.configured),
    },
    {
      id: 'safety',
      label: 'Write guardrails remain preview-first',
      ok: settings.guardrails?.defaultLevel !== 'dangerous',
    },
  ];
  const passed = checks.filter((check) => check.ok).length;
  const score = Math.round((passed / checks.length) * 100);
  const grade =
    score === 100
      ? 'A+'
      : score >= 90
        ? 'A'
        : score >= 80
          ? 'B'
          : score >= 70
            ? 'C'
            : score >= 60
              ? 'D'
              : 'F';
  const nextActions = [];

  if (!checks.find((check) => check.id === 'provider')?.ok) {
    nextActions.push('Set ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY, or run local Ollama.');
  }
  if (!checks.find((check) => check.id === 'memory')?.ok) {
    nextActions.push(
      'Enable memory in .stateset/settings.json for cross-session commerce context.',
    );
  }
  if (!checks.find((check) => check.id === 'runbooks')?.ok) {
    nextActions.push(`Create a workspace runbook with stateset agent runbook create "Daily ops".`);
  }
  if (!checks.find((check) => check.id === 'sessions')?.ok) {
    nextActions.push('Enable the session store so agent work can resume across runs.');
  }
  if (!checks.find((check) => check.id === 'channels')?.ok) {
    nextActions.push('Run stateset agent setup to create a local webchat channel config.');
  }
  if (!sessions.readable) {
    nextActions.push(`Install optional session backend support or inspect ${sessions.dbPath}.`);
  }
  if (!memory.exists) {
    nextActions.push(
      `Capture operational memory with stateset agent remember "..." before launch.`,
    );
  }

  return {
    score,
    grade,
    passed,
    total: checks.length,
    checks,
    nextActions,
    workspaceSkillDir,
  };
}

export function collectAgentOsStatus(options = {}) {
  const {
    env = process.env,
    limit = 5,
    memoryDir,
    sessionDbPath,
    workspaceSkillDir = DEFAULT_WORKSPACE_SKILLS_DIR,
    cwd = process.cwd(),
    settings: providedSettings = null,
  } = options;
  const safeLimit = normalizeLimit(limit);
  const settings = providedSettings || loadAgentSettings({}, { reload: true });
  const providers = providerReadiness(env);
  const { registry } = createRegistry({ workspaceSkillDir });
  const skills = summarizeSkills(registry, safeLimit);
  const resolvedMemoryDir =
    memoryDir || resolvePathFrom(cwd, settings.memory?.dir) || DEFAULT_AGENT_MEMORY_DIR;
  const memory = inspectMemory(resolvedMemoryDir);
  const sessions = inspectSessions(
    sessionDbPath || settings.sessionStore?.dbPath || DEFAULT_SESSION_DB_PATH,
    safeLimit,
  );
  const channels = inspectChannels({ env, settings, cwd });
  const readiness = buildReadiness({
    providers,
    settings,
    skills,
    memory,
    sessions,
    channels,
    workspaceSkillDir,
  });

  return {
    version: '1.0.0',
    generatedAt: new Date().toISOString(),
    defaultAgent: settings.agent?.default || 'customer-service',
    defaultProvider: settings.provider?.default || 'claude',
    defaultModel: settings.model?.default || DEFAULT_MODEL,
    agents: Object.keys(AGENTS).sort(),
    providers,
    settings: {
      memory: settings.memory || {},
      privacy: settings.privacy || {},
      guardrails: settings.guardrails || {},
      contextGuard: settings.contextGuard || {},
      sessionStore: settings.sessionStore || {},
    },
    skills,
    memory,
    sessions,
    channels,
    readiness,
    nextActions: readiness.nextActions,
  };
}

export async function setupAgentWorkspace(options = {}) {
  const cwd = options.cwd || process.cwd();
  const settingsPath = options.settingsPath || path.join(cwd, '.stateset', 'settings.json');
  const statesetDir = path.dirname(settingsPath);
  const workspaceSkillDir = options.workspaceSkillDir || path.join(statesetDir, 'skills');
  const memoryDir = options.memoryDir || path.join(statesetDir, 'memory');
  const memoryDbPath = options.memoryDbPath || path.join(statesetDir, 'memory.db');
  const sessionDbPath = options.sessionDbPath || path.join(statesetDir, 'agent-sessions.db');
  const channelConfigPath = options.channelConfigPath || path.join(statesetDir, 'channels.json');
  const {
    provider,
    agent,
    createRunbook = true,
    createChannelConfig = true,
    channel = 'webchat',
    runbookName = 'Launch Operations',
  } = options;
  const dryRun = options.dryRun === true;
  const existingSettings = readJsonFile(settingsPath);
  const existingChannelConfig = readJsonFile(channelConfigPath);
  const channelConfig = mergeChannelConfig(
    existingChannelConfig,
    buildChannelConfig({
      channel,
      dbPath: './store.db',
      agent: agent || existingSettings.agent?.default || 'customer-service',
    }),
  );
  const setupSettings = {
    agent: { default: 'customer-service' },
    provider: { default: 'claude' },
    model: { preferSession: true },
    memory: {
      enabled: true,
      useMarkdown: true,
      maxSummaries: 5,
      dir: relativeIfInside(cwd, memoryDir),
      dbPath: relativeIfInside(cwd, memoryDbPath),
    },
    sessionStore: {
      enabled: true,
      dbPath: relativeIfInside(cwd, sessionDbPath),
      maxSummaries: 5,
    },
    contextGuard: {
      enabled: true,
      warningThreshold: 0.7,
      compactThreshold: 0.8,
      abortThreshold: 0.95,
      reserveTokens: 4096,
    },
    privacy: {
      redactLogs: true,
      redactMemory: true,
      redactHistory: true,
      redactResponse: false,
    },
  };
  if (createChannelConfig) {
    setupSettings.channels = {
      configPath: relativeIfInside(cwd, channelConfigPath),
      default: channel,
    };
  }
  const settings = mergeSetupSettings(existingSettings, setupSettings, { agent, provider });
  const previous = pathExists(settingsPath) ? safeReadFile(settingsPath) : '';
  const next = stableJson(settings);
  const changes = [];

  if (!dryRun) {
    await fsp.mkdir(statesetDir, { recursive: true });
    await fsp.mkdir(workspaceSkillDir, { recursive: true });
    await ensureMemoryDirs(memoryDir);
    if (previous !== next) {
      await fsp.writeFile(settingsPath, next, 'utf-8');
    }
    if (createChannelConfig) {
      await fsp.writeFile(channelConfigPath, stableJson(channelConfig), 'utf-8');
    }
  }

  const settingsAction = previous ? (previous === next ? 'unchanged' : 'updated') : 'created';
  const dryRunSettingsAction =
    settingsAction === 'created'
      ? 'would-create'
      : settingsAction === 'updated'
        ? 'would-update'
        : settingsAction;
  changes.push({
    action: dryRun ? dryRunSettingsAction : settingsAction,
    path: settingsPath,
    type: 'settings',
  });
  changes.push({
    action: dryRun ? 'would-create' : 'ready',
    path: workspaceSkillDir,
    type: 'skills-dir',
  });
  changes.push({
    action: dryRun ? 'would-create' : 'ready',
    path: memoryDir,
    type: 'memory-dir',
  });

  if (createChannelConfig) {
    const previousChannel = pathExists(channelConfigPath) ? safeReadFile(channelConfigPath) : '';
    const nextChannel = stableJson(channelConfig);
    const channelAction = previousChannel
      ? previousChannel === nextChannel
        ? 'unchanged'
        : 'updated'
      : 'created';
    const dryRunChannelAction =
      channelAction === 'created'
        ? 'would-create'
        : channelAction === 'updated'
          ? 'would-update'
          : channelAction;
    changes.push({
      action: dryRun ? dryRunChannelAction : channelAction,
      path: channelConfigPath,
      type: 'channel-config',
    });
  }

  let runbook = null;
  if (createRunbook) {
    try {
      runbook = dryRun
        ? {
            created: false,
            name: `${RUNBOOK_PREFIX}-${slugify(runbookName)}`,
            path: path.join(
              workspaceSkillDir,
              `${RUNBOOK_PREFIX}-${slugify(runbookName)}`,
              'SKILL.md',
            ),
            workspaceDir: path.resolve(workspaceSkillDir),
          }
        : await createRunbookSkill({
            name: runbookName,
            description: 'launch readiness and daily commerce operations',
            workspaceDir: workspaceSkillDir,
            force: false,
          });
      changes.push({
        action: dryRun ? 'would-create' : 'created',
        path: runbook.path,
        type: 'runbook',
      });
    } catch (error) {
      if (!/already exists/i.test(error.message)) throw error;
      const runbookPath = path.join(
        workspaceSkillDir,
        `${RUNBOOK_PREFIX}-${slugify(runbookName)}`,
        'SKILL.md',
      );
      runbook = {
        created: false,
        name: `${RUNBOOK_PREFIX}-${slugify(runbookName)}`,
        path: runbookPath,
        workspaceDir: path.resolve(workspaceSkillDir),
      };
      changes.push({ action: 'unchanged', path: runbookPath, type: 'runbook' });
    }
  }

  return {
    dryRun,
    settingsPath,
    workspaceSkillDir,
    memoryDir,
    sessionDbPath,
    channelConfigPath,
    settings,
    channelConfig: createChannelConfig ? channelConfig : null,
    runbook,
    changes,
  };
}

export function inspectAgentContext(options = {}) {
  const status = collectAgentOsStatus(options);
  const contextGuard = status.settings.contextGuard || {};
  const sessions = status.sessions.recent.map((session) => ({
    sessionId: session.sessionId,
    agent: session.agent,
    model: session.model,
    provider: session.provider,
    totalTokens: Number(session.totalTokens || 0),
    compactionCount: Number(session.compactionCount || 0),
    updatedAt: session.updatedAt,
    lastError: session.lastError || session.lastErrorCode || null,
  }));
  const highTokenSessions = sessions
    .filter((session) => session.totalTokens > 0)
    .sort((a, b) => b.totalTokens - a.totalTokens)
    .slice(0, normalizeLimit(options.limit, 5));
  const totalCompactions = sessions.reduce((total, session) => total + session.compactionCount, 0);
  const warnings = [];

  if (!contextGuard.enabled) {
    warnings.push('Context guard is disabled.');
  }
  if (!status.settings.memory?.enabled) {
    warnings.push('Memory is disabled, so useful context will not persist across runs.');
  }
  if (status.sessions.failures.length > 0) {
    warnings.push(`${status.sessions.failures.length} recent session failure(s) need review.`);
  }
  if (highTokenSessions[0]?.totalTokens >= 100_000) {
    warnings.push('At least one recent session is token-heavy; compact or start a fresh session.');
  }

  return {
    generatedAt: status.generatedAt,
    contextGuard,
    memory: {
      enabled: Boolean(status.settings.memory?.enabled),
      useMarkdown: status.settings.memory?.useMarkdown !== false,
      dir: status.memory.dir,
      entries: status.memory.mainEntries,
    },
    sessions: {
      count: status.sessions.count,
      backend: status.sessions.backend,
      readable: status.sessions.readable,
      highTokenSessions,
      totalCompactions,
      failures: status.sessions.failures,
    },
    warnings,
  };
}

function formatCheck(ok) {
  return ok ? 'ok' : 'needs work';
}

export function formatAgentStatus(status) {
  const configuredProviders = status.providers
    .filter((provider) => provider.configured)
    .map((provider) => provider.id)
    .join(', ');
  const configuredChannels = status.channels
    .filter((channel) => channel.configured)
    .map((channel) => channel.name)
    .join(', ');
  const lines = [
    `StateSet Agent OS ${status.version}`,
    `Readiness: ${status.readiness.grade} (${status.readiness.score}/100, ${status.readiness.passed}/${status.readiness.total} checks)`,
    `Default: ${status.defaultAgent} via ${status.defaultProvider} (${status.defaultModel})`,
    `Providers: ${configuredProviders || 'none configured'}`,
    `Skills: ${status.skills.total} total (${status.skills.workspace} workspace, ${status.skills.installed} installed, ${status.skills.bundled} bundled)`,
    `Memory: ${status.settings.memory.enabled ? 'enabled' : 'disabled'}; ${status.memory.mainEntries} entries in ${status.memory.dir}`,
    `Sessions: ${status.sessions.count} recorded (${status.sessions.backend || 'none'})`,
    `Channels: ${configuredChannels || 'none configured'}`,
    '',
    'Checks:',
    ...status.readiness.checks.map((check) => `  - ${formatCheck(check.ok)}: ${check.label}`),
  ];

  if (status.nextActions.length > 0) {
    lines.push('', 'Next actions:', ...status.nextActions.map((action) => `  - ${action}`));
  }

  return lines.join('\n');
}

export function formatNextActions(status) {
  if (!status.nextActions.length) {
    return 'No blocking agent-product next actions detected.';
  }
  return ['Next actions:', ...status.nextActions.map((action) => `  - ${action}`)].join('\n');
}

export function formatSetupResult(result) {
  const lines = [result.dryRun ? 'Agent workspace setup plan:' : 'Agent workspace setup complete:'];
  for (const change of result.changes) {
    lines.push(`  - ${change.action}: ${change.path}`);
  }
  lines.push(
    '',
    'Enabled:',
    `  - memory: ${result.settings.memory.enabled ? 'enabled' : 'disabled'} (${result.settings.memory.dir})`,
    `  - session store: ${result.settings.sessionStore.enabled ? 'enabled' : 'disabled'} (${result.settings.sessionStore.dbPath})`,
    `  - context guard: ${result.settings.contextGuard.enabled ? 'enabled' : 'disabled'}`,
  );
  return lines.join('\n');
}

export function formatAgentContext(context) {
  const guard = context.contextGuard || {};
  const lines = [
    'Agent Context',
    `Guard: ${guard.enabled ? 'enabled' : 'disabled'} (warn ${guard.warningThreshold ?? 'n/a'}, compact ${guard.compactThreshold ?? 'n/a'}, abort ${guard.abortThreshold ?? 'n/a'})`,
    `Memory: ${context.memory.enabled ? 'enabled' : 'disabled'}; ${context.memory.entries} entries in ${context.memory.dir}`,
    `Sessions: ${context.sessions.count} recorded (${context.sessions.backend || 'none'}), ${context.sessions.totalCompactions} compactions`,
  ];

  if (context.sessions.highTokenSessions.length > 0) {
    lines.push('', 'High-token sessions:');
    for (const session of context.sessions.highTokenSessions) {
      lines.push(
        `  - ${session.sessionId}: ${session.totalTokens} tokens, ${session.compactionCount} compactions`,
      );
    }
  }

  if (context.warnings.length > 0) {
    lines.push('', 'Warnings:', ...context.warnings.map((warning) => `  - ${warning}`));
  }

  return lines.join('\n');
}

export function listAgentSkills(options = {}) {
  const {
    query = '',
    category,
    origin,
    limit = 25,
    workspaceSkillDir = DEFAULT_WORKSPACE_SKILLS_DIR,
  } = options;
  const safeLimit = normalizeLimit(limit, 25);
  const { registry } = createRegistry({ workspaceSkillDir });
  let skills = query ? registry.search(query) : registry.list({ category, origin });
  if (query && (category || origin)) {
    skills = skills.filter(
      (skill) => (!category || skill.category === category) && (!origin || skill.origin === origin),
    );
  }
  return skills.slice(0, safeLimit).map((skill) => ({
    name: skill.name,
    description: skill.description,
    category: skill.category,
    origin: skill.origin,
    tags: skill.tags,
    hasReferences: skill.hasReferences,
    hasScripts: skill.hasScripts,
  }));
}

export function formatSkillList(skills) {
  if (!skills.length) return 'No matching skills found.';
  const lines = [`Skills (${skills.length}):`];
  for (const skill of skills) {
    lines.push(
      `  - ${skill.name} [${skill.category}/${skill.origin}]: ${truncate(skill.description, 110)}`,
    );
  }
  return lines.join('\n');
}

export function listAgentSessions(options = {}) {
  const { limit = 10, sessionDbPath } = options;
  return inspectSessions(sessionDbPath || DEFAULT_SESSION_DB_PATH, normalizeLimit(limit, 10));
}

export function formatSessionList(sessionState) {
  if (!sessionState.readable) {
    return `Sessions unavailable: ${sessionState.reason || 'unknown error'}`;
  }
  if (!sessionState.recent.length) {
    return `No recorded agent sessions in ${sessionState.dbPath}.`;
  }
  const lines = [`Sessions (${sessionState.count} recorded):`];
  for (const session of sessionState.recent) {
    lines.push(
      `  - ${session.sessionId}: ${session.agent || 'agent'} ${session.provider || 'provider'} ${session.model || ''} updated ${formatAge(session.updatedAt)}`,
    );
  }
  return lines.join('\n');
}

export async function saveOperationalMemory(options = {}) {
  const {
    summary,
    facts = [],
    agent = 'agent-os',
    sessionId,
    memoryDir = DEFAULT_AGENT_MEMORY_DIR,
  } = options;
  if (!summary || !String(summary).trim()) {
    throw new Error('A memory summary is required.');
  }
  const store = new MarkdownMemoryStore({ memoryDir });
  const entry = {
    summary: String(summary).trim(),
    facts: facts.filter(Boolean).map(String),
    agent,
    sessionId,
  };
  await store.save(entry);
  return {
    saved: true,
    memoryDir,
    summary: entry.summary,
    facts: entry.facts,
    agent,
    sessionId: sessionId || null,
  };
}

export async function searchOperationalMemory(options = {}) {
  const { query = '', limit = 10, memoryDir = DEFAULT_AGENT_MEMORY_DIR } = options;
  const store = new MarkdownMemoryStore({ memoryDir });
  const safeLimit = normalizeLimit(limit, 10);
  if (query && String(query).trim()) {
    return store.search(String(query), safeLimit);
  }
  return store.getRecent(safeLimit);
}

export function formatMemoryList(entries, { query } = {}) {
  if (!entries.length) {
    return query ? `No memory entries matched "${query}".` : 'No operational memory entries found.';
  }
  const title = query ? `Memory matches for "${query}"` : 'Recent operational memory';
  const lines = [`${title} (${entries.length}):`];
  for (const entry of entries) {
    lines.push(`  - ${entry.timestamp || 'undated'}: ${truncate(entry.summary || entry.raw, 120)}`);
  }
  return lines.join('\n');
}

function runbookTemplate({ name, skillName, description }) {
  return `---
name: ${skillName}
description: ${description}
---
# ${name}

## Trigger
Use this runbook when the commerce agent needs a repeatable procedure for ${description}.

## Preconditions
- Confirm the requester, store context, and affected commerce objects.
- Keep writes in preview mode unless the operator explicitly enables apply mode.
- Check recent operational memory for related customer, order, inventory, or payment facts.

## Procedure
1. Restate the intended outcome and the safety boundary.
2. Gather the minimum required order, customer, inventory, payment, and policy context.
3. Execute read-only checks first.
4. Present the proposed mutation, expected side effects, and rollback path.
5. Apply the change only after approval.

## Safety
- Redact customer secrets, payment credentials, and private channel tokens.
- Prefer deterministic commerce tools over free-form shell actions.
- Escalate ambiguous financial, fulfillment, or policy decisions to a human operator.

## Memory Capture
- Record durable decisions with \`stateset agent remember\`.
- Include entity identifiers, final outcome, and follow-up obligations.
`;
}

export async function createRunbookSkill(options = {}) {
  const {
    name,
    description = 'a recurring commerce operations workflow',
    workspaceDir = DEFAULT_WORKSPACE_SKILLS_DIR,
    force = false,
  } = options;
  const slug = slugify(name);
  if (!slug) {
    throw new Error('A runbook name is required.');
  }
  const skillName = `${RUNBOOK_PREFIX}-${slug}`;
  const skillDir = path.resolve(workspaceDir, skillName);
  const skillPath = path.join(skillDir, 'SKILL.md');

  if (pathExists(skillPath) && !force) {
    throw new Error(`Runbook already exists at ${skillPath}. Re-run with --force to overwrite.`);
  }

  await fsp.mkdir(skillDir, { recursive: true });
  await fsp.writeFile(
    skillPath,
    runbookTemplate({ name: String(name).trim(), skillName, description }),
    'utf-8',
  );

  return {
    created: true,
    name: skillName,
    path: skillPath,
    workspaceDir: path.resolve(workspaceDir),
  };
}

export function formatRunbookCreated(result) {
  return `Created ${result.name} at ${result.path}`;
}
