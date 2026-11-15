/**
 * Agent directories.
 *
 * One directory per agent, outside the repository by default:
 *
 *   <dir>/agent.json      public identity: name, agent UUID, key id, public key
 *   <dir>/store.db        the agent's own iCommerce store (@stateset/embedded Commerce)
 *   <dir>/outbox.db       the CLI's VES outbox: signed board posts and their sync
 *                         state, plus the peer key cache and trust-on-first-use pins
 *   <dir>/.stateset/keys/<agentId>/signing-keys.json
 *                         the Ed25519 signing key (mode 0600), written by the
 *                         CLI's AgentKeyManager
 *
 * The outbox is a separate file from store.db on purpose. The Commerce engine
 * and better-sqlite3 each bundle their own SQLite; two SQLite libraries holding
 * the same file in one process defeat SQLite's POSIX advisory locking (closing
 * either library's descriptor drops the other's locks), which can lose writes.
 */

import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

import { AgentKeyManager } from '../../../cli/src/sync/keys.js';
import { createOutbox } from '../../../cli/src/sync/outbox.js';
import { bufferToHex } from '../../../cli/src/sync/crypto.js';
import { KEY_ALGORITHM_ED25519 } from '../../../cli/src/sync/pqc.js';
import { loadCommerce, loadDatabase } from './deps.mjs';

const AGENT_NAME = /^[a-z0-9][a-z0-9_-]{0,62}$/i;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const MANIFEST_VERSION = 1;

export function validateAgentName(name) {
  if (typeof name !== 'string' || !AGENT_NAME.test(name)) {
    throw new Error(
      'Agent name must be 1-63 letters, digits, "-" or "_", starting with a letter or digit',
    );
  }
  return name;
}

/**
 * @param {{ name: string, dir?: string, home: string }} options
 * @returns {string} absolute agent directory
 */
export function resolveAgentDir({ name, dir, home }) {
  validateAgentName(name);
  return path.resolve(dir ?? path.join(home, name));
}

export function agentFiles(dir) {
  return {
    dir,
    manifest: path.join(dir, 'agent.json'),
    store: path.join(dir, 'store.db'),
    outbox: path.join(dir, 'outbox.db'),
    configDir: path.join(dir, '.stateset'),
  };
}

function keyManagerFor(files) {
  return new AgentKeyManager(files.configDir, { securityProfile: 'legacy' });
}

function writeManifest(files, manifest) {
  const temp = `${files.manifest}.${process.pid}.tmp`;
  fs.writeFileSync(temp, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(temp, files.manifest);
}

export function loadManifest(dir, expectedName) {
  const files = agentFiles(dir);
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(files.manifest, 'utf8'));
  } catch (error) {
    if (error.code === 'ENOENT') {
      throw new Error(
        `No agent at ${dir}; run: board.mjs init --agent ${expectedName ?? '<name>'}${expectedName ? '' : ' --dir <path>'}`,
      );
    }
    throw new Error(`Cannot read ${files.manifest}: ${error.message}`);
  }
  if (manifest?.version !== MANIFEST_VERSION || !UUID.test(manifest.agentId ?? '')) {
    throw new Error(`${files.manifest} is not a board agent manifest`);
  }
  if (expectedName && manifest.name !== expectedName) {
    throw new Error(`${dir} belongs to agent "${manifest.name}", not "${expectedName}"`);
  }
  return manifest;
}

/**
 * Create an agent directory: store, outbox, signing key, manifest.
 * Idempotent: an existing agent directory is returned as is.
 *
 * @param {{ name: string, dir: string, agentId?: string }} options
 * @returns {Promise<{ created: boolean, manifest: object, files: object }>}
 */
export async function initAgent({ name, dir, agentId }) {
  validateAgentName(name);
  if (agentId !== undefined && !UUID.test(agentId)) throw new Error('--agent-id must be a UUID');
  const files = agentFiles(dir);
  if (fs.existsSync(files.manifest)) {
    return { created: false, manifest: loadManifest(dir, name), files };
  }
  if (fs.existsSync(dir) && fs.readdirSync(dir).length > 0) {
    throw new Error(`${dir} exists and is not an agent directory; choose another --dir`);
  }
  fs.mkdirSync(dir, { recursive: true, mode: 0o700 });

  const id = (agentId ?? randomUUID()).toLowerCase();

  // The agent's own iCommerce instance. Opening runs the engine's migrations.
  const Commerce = loadCommerce();
  const commerce = await Commerce.open(files.store);
  await commerce.close();

  // The VES outbox schema, in its own SQLite file (see the module comment).
  const Database = loadDatabase();
  const db = new Database(files.outbox);
  try {
    db.pragma('journal_mode = WAL');
    createOutbox(db, { configDir: files.configDir, keyManager: keyManagerFor(files) });
  } finally {
    db.close();
  }

  const signingKey = await keyManagerFor(files).generateSigningKey(id);
  fs.chmodSync(files.configDir, 0o700);

  const manifest = {
    version: MANIFEST_VERSION,
    name,
    agentId: id,
    keyId: signingKey.keyId,
    keyAlgorithm: signingKey.keyAlgorithm ?? KEY_ALGORITHM_ED25519,
    publicKey: bufferToHex(signingKey.publicKey),
    createdAt: new Date().toISOString(),
    registrations: [],
  };
  writeManifest(files, manifest);
  return { created: true, manifest, files };
}

/**
 * Open an agent for posting or reading: manifest, outbox, key manager.
 * The caller must call `close()`.
 */
export async function openAgent(dir, expectedName) {
  const manifest = loadManifest(dir, expectedName);
  const files = agentFiles(dir);
  const keyManager = keyManagerFor(files);
  const signingKey = await keyManager.getCurrentSigningKey(manifest.agentId);
  if (!signingKey) throw new Error(`Agent "${manifest.name}" has no usable signing key`);
  if (bufferToHex(signingKey.publicKey) !== manifest.publicKey) {
    throw new Error(
      `Agent "${manifest.name}": signing key does not match agent.json; re-run init in a new directory`,
    );
  }
  const Database = loadDatabase();
  const db = new Database(files.outbox);
  db.pragma('busy_timeout = 5000');
  const outbox = createOutbox(db, { configDir: files.configDir, keyManager });
  return {
    manifest,
    files,
    db,
    outbox,
    keyManager,
    signingKey: { keyId: signingKey.keyId, publicKey: bufferToHex(signingKey.publicKey) },
    saveManifest: (next) => writeManifest(files, next),
    close: () => db.close(),
  };
}

/** Record a successful key registration against one sequencer + tenant. */
export function recordRegistration(agent, { sequencerUrl, tenantId }) {
  const registrations = (agent.manifest.registrations ?? []).filter(
    (r) => !(r.sequencerUrl === sequencerUrl && r.tenantId === tenantId),
  );
  registrations.push({
    sequencerUrl,
    tenantId,
    keyId: agent.signingKey.keyId,
    registeredAt: new Date().toISOString(),
  });
  agent.manifest = { ...agent.manifest, registrations };
  agent.saveManifest(agent.manifest);
}

export function findRegistration(manifest, { sequencerUrl, tenantId }) {
  return (
    (manifest.registrations ?? []).find(
      (r) =>
        r.sequencerUrl === sequencerUrl && r.tenantId === tenantId && r.keyId === manifest.keyId,
    ) ?? null
  );
}

/** Counts from the agent's own iCommerce store. */
export async function storeSummary(files) {
  const Commerce = loadCommerce();
  const commerce = await Commerce.open(files.store);
  try {
    const [customers, orders, products] = await Promise.all([
      commerce.customers.count(),
      commerce.orders.count(),
      commerce.products.count(),
    ]);
    return { customers, orders, products };
  } finally {
    await commerce.close();
  }
}

/** Every agent manifest directly under `home`, sorted by name. */
export function listAgents(home) {
  if (!fs.existsSync(home)) return [];
  return fs
    .readdirSync(home, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(home, entry.name))
    .filter((dir) => fs.existsSync(agentFiles(dir).manifest))
    .map((dir) => ({ dir, manifest: loadManifest(dir) }))
    .sort((a, b) => a.manifest.name.localeCompare(b.manifest.name));
}
