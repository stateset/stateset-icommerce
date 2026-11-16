#!/usr/bin/env node
/**
 * Agent message board over a VES sequencer.
 *
 * Several agents, each with its own iCommerce store and its own Ed25519 VES
 * key, post signed messages to one shared sequencer and read the sequenced
 * log back with every message's signature (and, once committed, Merkle
 * inclusion) checked. See README.md in this directory.
 *
 *   board.mjs init     --agent <name> [--dir <path>] [--agent-id <uuid>]
 *   board.mjs register --agent <name> [--dir <path>] [--force]
 *   board.mjs post     --agent <name> --thread <id> "<text>"
 *                      [--reply-to <seq>] [--state <json>] [--ref <x>]... [--store-summary]
 *   board.mjs read     [--thread <id>] [--agent <reader>] [--roster <file>] [--no-proofs]
 *                      [--from <seq>] [--json]
 *   board.mjs commit
 *   board.mjs whoami   [--agent <name>]
 *   board.mjs roster   [--agents a,b,c]
 *
 * Global: --config <file> (JSON with SEQUENCER_* keys), --json.
 * Configuration comes only from the environment or --config; see lib/config.mjs.
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { boardHome, isInside, readRawSettings, sequencerSettings } from './lib/config.mjs';
import {
  findRegistration,
  initAgent,
  listAgents,
  loadManifest,
  openAgent,
  recordRegistration,
  resolveAgentDir,
  storeSummary,
} from './lib/agent.mjs';
import {
  commitNext,
  loadRoster,
  postMessage,
  readBoard,
  registerKey,
  renderBoard,
} from './lib/board.mjs';

// The CLI's crypto helpers report native-fallback noise on console.debug,
// which is stdout in Node and would corrupt --json output.
const debug = process.env.BOARD_DEBUG === '1';
console.debug = debug ? (...args) => console.error('[debug]', ...args) : () => {};

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const USAGE = `usage:
  board.mjs init     --agent <name> [--dir <path>] [--agent-id <uuid>]
  board.mjs register --agent <name> [--dir <path>] [--force]
  board.mjs post     --agent <name> --thread <id> "<text>" [--reply-to <seq>] [--state <json>]
                     [--ref <x>]... [--store-summary]
  board.mjs read     [--thread <id>] [--agent <reader>] [--roster <file>] [--no-proofs] [--from <seq>]
  board.mjs commit
  board.mjs whoami   [--agent <name>]
  board.mjs roster   [--agents a,b,c]
global options: --config <file>  --dir <path>  --json
environment: SEQUENCER_URL SEQUENCER_API_KEY SEQUENCER_TENANT_ID SEQUENCER_STORE_ID
             [SEQUENCER_PUBLIC_KEY] [BOARD_HOME] [BOARD_ROSTER]`;

class UsageError extends Error {}

const OPTIONS = {
  agent: { type: 'string' },
  agents: { type: 'string' },
  'agent-id': { type: 'string' },
  config: { type: 'string' },
  dir: { type: 'string' },
  force: { type: 'boolean' },
  from: { type: 'string' },
  help: { type: 'boolean', short: 'h' },
  json: { type: 'boolean' },
  'no-proofs': { type: 'boolean' },
  ref: { type: 'string', multiple: true },
  'reply-to': { type: 'string' },
  roster: { type: 'string' },
  state: { type: 'string' },
  'store-summary': { type: 'boolean' },
  thread: { type: 'string' },
};

const ALLOWED = {
  init: ['agent', 'dir', 'agent-id'],
  register: ['agent', 'dir', 'force'],
  post: ['agent', 'dir', 'thread', 'reply-to', 'state', 'ref', 'store-summary'],
  read: ['thread', 'agent', 'dir', 'roster', 'no-proofs', 'from'],
  commit: [],
  whoami: ['agent', 'dir'],
  roster: ['agents'],
};

function parse(argv) {
  let parsed;
  try {
    parsed = parseArgs({ args: argv, options: OPTIONS, allowPositionals: true, strict: true });
  } catch (error) {
    throw new UsageError(error.message);
  }
  const [command, ...rest] = parsed.positionals;
  const { values } = parsed;
  if (values.help || !command) return { command: 'help', values, rest };
  if (!ALLOWED[command]) throw new UsageError(`unknown command "${command}"`);
  for (const key of Object.keys(values)) {
    if (['config', 'json', 'help'].includes(key)) continue;
    if (!ALLOWED[command].includes(key)) {
      throw new UsageError(`--${key} is not an option of "${command}"`);
    }
  }
  if (command !== 'post' && rest.length) {
    throw new UsageError(`unexpected argument "${rest[0]}" for "${command}"`);
  }
  return { command, values, rest };
}

function positiveInt(value, flag) {
  if (value === undefined) return undefined;
  if (!/^[1-9][0-9]*$/.test(value) || !Number.isSafeInteger(Number(value))) {
    throw new UsageError(`${flag} must be a positive integer`);
  }
  return Number(value);
}

function requireAgent(values) {
  if (!values.agent) throw new UsageError('--agent <name> is required');
  return values.agent;
}

function agentDir(values, raw) {
  const dir = resolveAgentDir({
    name: requireAgent(values),
    dir: values.dir,
    home: boardHome(raw),
  });
  if (isInside(REPO_ROOT, dir)) {
    console.error(
      `warning: ${dir} is inside the repository; agent directories hold private keys and belong outside it`,
    );
  }
  return dir;
}

function out(values, json, text) {
  process.stdout.write(`${values.json ? JSON.stringify(json, null, 2) : text}\n`);
}

function hintFor(error) {
  // SequencerClient request failures carry the HTTP status.
  const status = error?.status ?? null;
  if (status === 401) return 'check SEQUENCER_API_KEY';
  if (status === 403)
    return 'the API key lacks permission for this tenant/store (key registration needs an admin key)';
  return null;
}

// ---------------------------------------------------------------------------

async function cmdInit(values, raw) {
  const dir = agentDir(values, raw);
  const { created, manifest } = await initAgent({
    name: values.agent,
    dir,
    agentId: values['agent-id'],
  });
  const summary = { created, dir, ...manifest };
  out(
    values,
    summary,
    [
      created
        ? `initialized agent "${manifest.name}" at ${dir}`
        : `agent "${manifest.name}" already exists at ${dir}`,
      `  agent id   ${manifest.agentId}`,
      `  key id     ${manifest.keyId}`,
      `  public key ${manifest.publicKey}`,
      `  store      ${path.join(dir, 'store.db')}`,
    ].join('\n'),
  );
}

async function cmdRegister(values, raw) {
  const dir = agentDir(values, raw);
  const settings = sequencerSettings(raw);
  const agent = await openAgent(dir, values.agent);
  try {
    const target = { sequencerUrl: settings.url, tenantId: settings.tenantId };
    const existing = findRegistration(agent.manifest, target);
    if (existing && !values.force) {
      out(
        values,
        { registered: true, skipped: true, ...existing, agentId: agent.manifest.agentId },
        `agent "${agent.manifest.name}" key ${existing.keyId} was already registered at ${existing.registeredAt} (--force re-sends)`,
      );
      return;
    }
    try {
      await registerKey({ agent, settings });
    } catch (error) {
      if (error?.status === 500) {
        error.message +=
          ' (the sequencer answers 500 when this agent and key id are already registered)';
      }
      throw error;
    }
    recordRegistration(agent, target);
    out(
      values,
      {
        registered: true,
        skipped: false,
        agentId: agent.manifest.agentId,
        keyId: agent.signingKey.keyId,
      },
      `registered agent "${agent.manifest.name}" (${agent.manifest.agentId}) key ${agent.signingKey.keyId} with ${settings.url}`,
    );
  } finally {
    agent.close();
  }
}

async function cmdPost(values, raw, rest) {
  const dir = agentDir(values, raw);
  if (!values.thread) throw new UsageError('--thread <id> is required');
  if (rest.length !== 1) throw new UsageError('post takes exactly one message argument (quote it)');
  let state;
  if (values.state !== undefined) {
    try {
      state = JSON.parse(values.state);
    } catch (error) {
      throw new UsageError(`--state must be JSON: ${error.message}`);
    }
  }
  const settings = sequencerSettings(raw);
  const agent = await openAgent(dir, values.agent);
  try {
    if (values['store-summary']) {
      if (
        state !== undefined &&
        (typeof state !== 'object' || state === null || Array.isArray(state))
      ) {
        throw new UsageError('--store-summary needs --state to be a JSON object (or omitted)');
      }
      state = { ...(state ?? {}), store: await storeSummary(agent.files) };
    }
    const result = await postMessage({
      agent,
      settings,
      thread: values.thread,
      text: rest[0],
      replyTo: positiveInt(values['reply-to'], '--reply-to'),
      refs: values.ref,
      state,
    });
    let text;
    if (result.status === 'sequenced') {
      text = `posted #${result.sequenceNumber} to thread "${result.thread}" (event ${result.eventId})`;
    } else if (result.status === 'rejected') {
      text = `REJECTED by the sequencer: ${result.reason}${result.message && !result.reason.includes(result.message) ? ` (${result.message})` : ''}`;
    } else {
      text = `not sequenced; kept in the outbox for the next post: ${result.error}`;
    }
    for (const other of result.others ?? []) {
      text += `\n  also flushed earlier event ${other.eventId}: ${other.status}${other.sequenceNumber ? ` #${other.sequenceNumber}` : ''}${other.reason ? ` (${other.reason})` : ''}`;
    }
    out(values, result, text);
    if (result.status !== 'sequenced') process.exitCode = 1;
  } finally {
    agent.close();
  }
}

async function cmdRead(values, raw) {
  const settings = sequencerSettings(raw);
  const rosterPath = values.roster ?? raw.BOARD_ROSTER;
  const roster = rosterPath ? loadRoster(rosterPath) : null;
  const readerAgent = values.agent ? await openAgent(agentDir(values, raw), values.agent) : null;
  try {
    const board = await readBoard({
      settings,
      thread: values.thread,
      readerAgent,
      roster,
      proofs: !values['no-proofs'],
      from: positiveInt(values.from, '--from') ?? 1,
    });
    out(values, board, renderBoard(board));
  } finally {
    readerAgent?.close();
  }
}

async function cmdCommit(values, raw) {
  const result = await commitNext({ settings: sequencerSettings(raw) });
  out(
    values,
    result,
    result.created
      ? `committed #${result.commitment.sequenceStart}..#${result.commitment.sequenceEnd} as batch ${result.commitment.batchId}`
      : `nothing to commit: head #${result.headSequence} is already committed`,
  );
}

function sequencerSummary(raw) {
  try {
    const s = sequencerSettings(raw, { requireApiKey: false });
    return {
      url: s.url,
      tenantId: s.tenantId,
      storeId: s.storeId,
      apiKey: raw.SEQUENCER_API_KEY ? 'set' : 'not set',
      sequencerPublicKey: s.sequencerPublicKey ? 'set' : 'not set',
    };
  } catch (error) {
    return { error: error.message, apiKey: raw.SEQUENCER_API_KEY ? 'set' : 'not set' };
  }
}

async function cmdWhoami(values, raw) {
  const home = boardHome(raw);
  const sequencer = sequencerSummary(raw);
  if (!values.agent) {
    const agents = listAgents(home).map(({ dir, manifest }) => ({
      name: manifest.name,
      agentId: manifest.agentId,
      dir,
    }));
    out(
      values,
      { home, sequencer, agents },
      [
        `board home  ${home}`,
        `sequencer   ${sequencer.url ?? `(${sequencer.error})`}  api key ${sequencer.apiKey}`,
        agents.length ? 'agents:' : 'no agents yet (board.mjs init --agent <name>)',
        ...agents.map((a) => `  ${a.name.padEnd(16)} ${a.agentId}  ${a.dir}`),
      ].join('\n'),
    );
    return;
  }
  const dir = agentDir(values, raw);
  const agent = await openAgent(dir, values.agent);
  try {
    const stats = agent.outbox.getStats();
    const store = await storeSummary(agent.files);
    const summary = {
      agent: {
        name: agent.manifest.name,
        agentId: agent.manifest.agentId,
        keyId: agent.signingKey.keyId,
        publicKey: agent.signingKey.publicKey,
        dir,
        createdAt: agent.manifest.createdAt,
        registrations: agent.manifest.registrations ?? [],
      },
      store,
      outbox: {
        total: stats.total,
        pending: stats.pending,
        synced: stats.synced,
        rejected: stats.rejected,
      },
      sequencer,
    };
    out(
      values,
      summary,
      [
        `agent       ${summary.agent.name} (${summary.agent.agentId})`,
        `signing key ${summary.agent.keyId}: ${summary.agent.publicKey}`,
        `directory   ${dir}`,
        `store       ${store.customers} customers, ${store.orders} orders, ${store.products} products`,
        `outbox      ${stats.synced} sequenced, ${stats.pending} pending, ${stats.rejected} rejected`,
        `registered  ${summary.agent.registrations.map((r) => `${r.sequencerUrl} (key ${r.keyId})`).join(', ') || 'nowhere yet'}`,
        `sequencer   ${sequencer.url ?? `(${sequencer.error})`}  api key ${sequencer.apiKey}`,
      ].join('\n'),
    );
  } finally {
    agent.close();
  }
}

function cmdRoster(values, raw) {
  const home = boardHome(raw);
  const wanted = values.agents
    ? values.agents
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
    : null;
  let entries;
  if (wanted) {
    entries = wanted.map((name) => {
      const dir = resolveAgentDir({ name, home });
      return { dir, manifest: loadManifest(dir, name) };
    });
  } else {
    entries = listAgents(home);
  }
  const roster = {
    version: 1,
    agents: entries.map(({ manifest }) => ({
      name: manifest.name,
      agentId: manifest.agentId,
      keyId: manifest.keyId,
      publicKey: manifest.publicKey,
    })),
  };
  process.stdout.write(`${JSON.stringify(roster, null, 2)}\n`);
}

async function main(argv) {
  const { command, values, rest } = parse(argv);
  if (command === 'help') {
    process.stdout.write(`${USAGE}\n`);
    return;
  }
  const raw = readRawSettings({ configPath: values.config });
  switch (command) {
    case 'init':
      return cmdInit(values, raw);
    case 'register':
      return cmdRegister(values, raw);
    case 'post':
      return cmdPost(values, raw, rest);
    case 'read':
      return cmdRead(values, raw);
    case 'commit':
      return cmdCommit(values, raw);
    case 'whoami':
      return cmdWhoami(values, raw);
    case 'roster':
      return cmdRoster(values, raw);
    default:
      throw new UsageError(`unknown command "${command}"`);
  }
}

main(process.argv.slice(2)).catch((error) => {
  if (error instanceof UsageError) {
    console.error(`board: ${error.message}\n${USAGE}`);
    process.exitCode = 2;
    return;
  }
  const hint = hintFor(error);
  console.error(`board: error: ${error.message}${hint ? `\n  hint: ${hint}` : ''}`);
  if (debug && error.stack) console.error(error.stack);
  process.exitCode = 1;
});
