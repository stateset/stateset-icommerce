#!/usr/bin/env node

/**
 * stateset-agent - Agent OS shell for StateSet iCommerce.
 *
 * Usage:
 *   stateset agent setup
 *   stateset agent status [--json]
 *   stateset agent context
 *   stateset agent next
 *   stateset agent skills [query]
 *   stateset agent sessions
 *   stateset agent memory [query]
 *   stateset agent remember "Daily cutoff moved to 3pm" --fact "Warehouse notified"
 *   stateset agent runbook create "Daily ops" --description "daily commerce operations"
 */

import { parseArgs } from 'node:util';
import fs from 'node:fs';
import {
  collectAgentOsStatus,
  createRunbookSkill,
  formatAgentContext,
  formatAgentStatus,
  formatMemoryList,
  formatNextActions,
  formatRunbookCreated,
  formatSessionList,
  formatSetupResult,
  formatSkillList,
  inspectAgentContext,
  listAgentSessions,
  listAgentSkills,
  saveOperationalMemory,
  setupAgentWorkspace,
  searchOperationalMemory,
} from '../src/agent-os.js';
import { CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet Agent OS v${CLI_VERSION}

USAGE:
  stateset agent [command] [options]
  stateset-agent [command] [options]

COMMANDS:
  setup                   Initialize workspace agent settings, memory dirs, and launch runbook
  status                  Show agent readiness, providers, skills, memory, sessions, channels
  context                 Show context guard, memory, compaction, and token-heavy sessions
  next                    Show prioritized next actions
  skills [query]          List or search commerce skills and workspace runbooks
  sessions                List recent agent sessions
  memory [query]          Search memory, or show recent memory when query is omitted
  memory search <query>   Search operational memory
  memory recent           Show recent operational memory
  remember <summary>      Save an operational memory entry
  runbook create <name>   Create a workspace SKILL.md runbook

OPTIONS:
  --json                  Output JSON
  --output <file>         Write output to a file
  --limit <n>             Maximum rows to return
  --memory-dir <path>     Override memory directory
  --memory-db <path>      Override memory SQLite path for setup
  --session-db <path>     Override agent session database path
  --settings-path <path>  Override workspace settings path for setup
  --channels-config <path> Override channel config path for setup/status
  --workspace-dir <path>  Override workspace skills directory
  --category <name>       Filter skills by category
  --origin <name>         Filter skills by origin: workspace, installed, bundled
  --provider <name>       Provider default for setup
  --default-agent <name>  Agent default for setup
  --channel <name>        Local channel to enable during setup (default: webchat)
  --description <text>    Runbook description
  --fact <text>           Add a memory fact; may be repeated
  --agent <name>          Agent name for memory capture
  --session <id>          Session ID for memory capture
  --dry-run               Preview setup changes without writing files
  --skip-runbook          Do not create the launch operations runbook during setup
  --skip-channel          Do not create a local channel config during setup
  --force                 Overwrite an existing runbook
  --help, -h              Show this help
  --version, -v           Show version
`;

const { values: flags, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    json: { type: 'boolean', default: false },
    output: { type: 'string' },
    limit: { type: 'string' },
    'memory-dir': { type: 'string' },
    'memory-db': { type: 'string' },
    'session-db': { type: 'string' },
    'settings-path': { type: 'string' },
    'channels-config': { type: 'string' },
    'workspace-dir': { type: 'string' },
    category: { type: 'string' },
    origin: { type: 'string' },
    provider: { type: 'string' },
    'default-agent': { type: 'string' },
    channel: { type: 'string' },
    description: { type: 'string' },
    fact: { type: 'string', multiple: true, default: [] },
    agent: { type: 'string' },
    session: { type: 'string' },
    'dry-run': { type: 'boolean', default: false },
    'skip-runbook': { type: 'boolean', default: false },
    'skip-channel': { type: 'boolean', default: false },
    force: { type: 'boolean', default: false },
    help: { type: 'boolean', short: 'h', default: false },
    version: { type: 'boolean', short: 'v', default: false },
  },
});

if (flags.version) {
  console.log(`@stateset/cli v${CLI_VERSION}`);
  process.exit(0);
}

if (flags.help) {
  console.log(HELP.trim());
  process.exit(0);
}

const command = positionals[0] || 'status';
const outputPath = flags.output || null;
if (outputPath) {
  flags.json = true;
}

function getLimit(fallback = 10) {
  const numeric = Number.parseInt(String(flags.limit || ''), 10);
  return Number.isFinite(numeric) && numeric > 0 ? numeric : fallback;
}

function commonOptions(fallbackLimit = 10) {
  const env = flags['channels-config']
    ? { ...process.env, STATESET_CHANNELS_CONFIG: flags['channels-config'] }
    : process.env;
  return {
    env,
    limit: getLimit(fallbackLimit),
    memoryDir: flags['memory-dir'],
    sessionDbPath: flags['session-db'],
    workspaceSkillDir: flags['workspace-dir'],
    cwd: process.cwd(),
  };
}

function writeOutput(payload, formatter) {
  const output = flags.json ? JSON.stringify(payload, null, 2) : formatter(payload);
  if (outputPath) {
    fs.writeFileSync(outputPath, output.endsWith('\n') ? output : `${output}\n`);
    return;
  }
  console.log(output);
}

function writeError(error) {
  const message = error?.message || String(error);
  if (flags.json) {
    console.error(JSON.stringify({ error: message }, null, 2));
  } else {
    console.error(message);
  }
}

function joinPositionals(startIndex) {
  return positionals.slice(startIndex).join(' ').trim();
}

async function runStatus() {
  const status = collectAgentOsStatus(commonOptions(5));
  writeOutput(status, formatAgentStatus);
}

async function runSetup() {
  const result = await setupAgentWorkspace({
    settingsPath: flags['settings-path'],
    workspaceSkillDir: flags['workspace-dir'],
    memoryDir: flags['memory-dir'],
    memoryDbPath: flags['memory-db'],
    sessionDbPath: flags['session-db'],
    channelConfigPath: flags['channels-config'],
    provider: flags.provider,
    agent: flags['default-agent'],
    channel: flags.channel,
    createRunbook: !flags['skip-runbook'],
    createChannelConfig: !flags['skip-channel'],
    runbookName: joinPositionals(1) || 'Launch Operations',
    dryRun: flags['dry-run'],
  });
  writeOutput(result, formatSetupResult);
}

async function runContext() {
  const context = inspectAgentContext(commonOptions(5));
  writeOutput(context, formatAgentContext);
}

async function runNext() {
  const status = collectAgentOsStatus(commonOptions(5));
  writeOutput(
    {
      readiness: status.readiness,
      nextActions: status.nextActions,
    },
    () => formatNextActions(status),
  );
}

async function runSkills() {
  const query = joinPositionals(1);
  const skills = listAgentSkills({
    query,
    category: flags.category,
    origin: flags.origin,
    limit: getLimit(25),
    workspaceSkillDir: flags['workspace-dir'],
  });
  writeOutput(skills, formatSkillList);
}

async function runSessions() {
  const sessions = listAgentSessions({
    limit: getLimit(10),
    sessionDbPath: flags['session-db'],
  });
  writeOutput(sessions, formatSessionList);
}

async function runMemory() {
  let query = joinPositionals(1);
  if (positionals[1] === 'search') {
    query = joinPositionals(2);
  } else if (positionals[1] === 'recent') {
    query = '';
  }
  const entries = await searchOperationalMemory({
    query,
    limit: getLimit(10),
    memoryDir: flags['memory-dir'],
  });
  writeOutput(entries, (payload) => formatMemoryList(payload, { query }));
}

async function runRemember() {
  const summary = joinPositionals(command === 'remember' ? 1 : 2);
  const result = await saveOperationalMemory({
    summary,
    facts: flags.fact,
    agent: flags.agent,
    sessionId: flags.session,
    memoryDir: flags['memory-dir'],
  });
  writeOutput(result, (payload) => `Saved operational memory: ${payload.summary}`);
}

async function runRunbook() {
  if (positionals[1] !== 'create') {
    throw new Error('Usage: stateset agent runbook create <name> [--description <text>]');
  }
  const name = joinPositionals(2);
  const result = await createRunbookSkill({
    name,
    description: flags.description,
    workspaceDir: flags['workspace-dir'],
    force: flags.force,
  });
  writeOutput(result, formatRunbookCreated);
}

try {
  if (command === 'setup' || command === 'init') {
    await runSetup();
  } else if (command === 'status') {
    await runStatus();
  } else if (command === 'context') {
    await runContext();
  } else if (command === 'next') {
    await runNext();
  } else if (command === 'skills') {
    await runSkills();
  } else if (command === 'sessions') {
    await runSessions();
  } else if (command === 'memory') {
    if (positionals[1] === 'remember') {
      await runRemember();
    } else {
      await runMemory();
    }
  } else if (command === 'remember') {
    await runRemember();
  } else if (command === 'runbook') {
    await runRunbook();
  } else {
    throw new Error(`Unknown stateset agent command "${command}". Run stateset agent --help.`);
  }
} catch (error) {
  writeError(error);
  process.exit(1);
}
