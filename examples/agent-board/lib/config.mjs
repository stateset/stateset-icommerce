/**
 * Board configuration.
 *
 * Sequencer settings come ONLY from environment variables or from a JSON file
 * whose path is given on the command line (`--config <path>`). Nothing here
 * reads a credential file from a default location, and nothing here ever
 * writes the API key anywhere or includes it in a message.
 *
 * Recognised keys (environment variable names; the config file uses the same
 * names as JSON keys, and its values win over the environment):
 *
 *   SEQUENCER_URL         origin of the sequencer, e.g. https://seq.example.com
 *   SEQUENCER_API_KEY     an `ss_...` API key (sent by the CLI client as
 *                         `Authorization: ApiKey <key>`)
 *   SEQUENCER_TENANT_ID   tenant UUID
 *   SEQUENCER_STORE_ID    store UUID -- the board is one store's event stream
 *   SEQUENCER_PUBLIC_KEY  optional: the sequencer's Ed25519 public key (hex),
 *                         needed to verify the signed agent key directory
 *   BOARD_HOME            optional: where agent directories live
 *                         (default: <os.tmpdir()>/stateset-agent-board)
 *   BOARD_ROSTER          optional: path to a roster of agent public keys,
 *                         used only when the key directory cannot be used
 */

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { normalizeSequencerPublicKey } from '../../../cli/src/sync/config.js';

export const CONFIG_KEYS = Object.freeze([
  'SEQUENCER_URL',
  'SEQUENCER_API_KEY',
  'SEQUENCER_TENANT_ID',
  'SEQUENCER_STORE_ID',
  'SEQUENCER_PUBLIC_KEY',
  'BOARD_HOME',
  'BOARD_ROSTER',
]);

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '[::1]', '::1']);

/** Default parent directory for agent directories: outside any repository. */
export function defaultBoardHome() {
  return path.join(os.tmpdir(), 'stateset-agent-board');
}

/**
 * Read the raw settings: config file (if any) over the environment.
 *
 * @param {{ configPath?: string, env?: NodeJS.ProcessEnv }} [options]
 * @returns {Record<string, string|undefined>}
 */
export function readRawSettings({ configPath, env = process.env } = {}) {
  const settings = {};
  for (const key of CONFIG_KEYS) {
    if (typeof env[key] === 'string' && env[key].trim()) settings[key] = env[key].trim();
  }
  if (configPath) {
    let parsed;
    try {
      parsed = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    } catch (error) {
      // Never echo file contents: the file may hold the API key.
      throw new Error(`Cannot read config file ${configPath}: ${error.code ?? 'invalid JSON'}`);
    }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error(`Config file ${configPath} must hold a JSON object`);
    }
    for (const key of Object.keys(parsed)) {
      if (!CONFIG_KEYS.includes(key)) {
        throw new Error(
          `Config file ${configPath} has unknown key "${key}"; expected ${CONFIG_KEYS.join(', ')}`,
        );
      }
      const value = parsed[key];
      if (value === null || value === undefined || value === '') continue;
      if (typeof value !== 'string') throw new Error(`Config key ${key} must be a string`);
      settings[key] = value.trim();
    }
  }
  return settings;
}

/**
 * Normalise a sequencer URL to its origin and refuse plaintext HTTP to
 * anything but loopback (a local mock or a port-forward).
 *
 * @param {string} raw
 * @returns {{ url: string, insecureLoopback: boolean }}
 */
export function normalizeSequencerUrl(raw) {
  let parsed;
  try {
    parsed = new URL(raw);
  } catch {
    throw new Error(`SEQUENCER_URL is not a valid URL: ${raw}`);
  }
  if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
    throw new Error(`SEQUENCER_URL must be http(s); got ${parsed.protocol}`);
  }
  if (parsed.username || parsed.password) {
    throw new Error('SEQUENCER_URL must not embed credentials; use SEQUENCER_API_KEY');
  }
  const insecureLoopback = parsed.protocol === 'http:';
  if (insecureLoopback && !LOOPBACK_HOSTS.has(parsed.hostname)) {
    throw new Error(
      `Refusing plaintext http:// to ${parsed.hostname}: use https://, or http:// only for a loopback mock`,
    );
  }
  // The CLI client appends /api/v1/...; accept an origin given with /api too.
  let pathname = parsed.pathname.replace(/\/+$/, '');
  if (pathname === '/api') pathname = '';
  if (pathname) {
    throw new Error(`SEQUENCER_URL must be the sequencer origin (no path); got ${parsed.pathname}`);
  }
  return { url: `${parsed.protocol}//${parsed.host}`, insecureLoopback };
}

/**
 * Settings needed by every command, sequencer or not.
 *
 * @param {Record<string, string|undefined>} raw
 */
export function boardHome(raw) {
  return path.resolve(raw.BOARD_HOME || defaultBoardHome());
}

/**
 * Validated sequencer settings. Throws a message naming the missing
 * variable, never its value.
 *
 * @param {Record<string, string|undefined>} raw
 * @param {{ requireApiKey?: boolean }} [options]
 */
export function sequencerSettings(raw, { requireApiKey = true } = {}) {
  const missing = ['SEQUENCER_URL', 'SEQUENCER_TENANT_ID', 'SEQUENCER_STORE_ID'].filter(
    (key) => !raw[key],
  );
  if (requireApiKey && !raw.SEQUENCER_API_KEY) missing.push('SEQUENCER_API_KEY');
  if (missing.length) {
    throw new Error(
      `Missing sequencer configuration: ${missing.join(', ')} (set them in the environment or a --config file)`,
    );
  }
  for (const key of ['SEQUENCER_TENANT_ID', 'SEQUENCER_STORE_ID']) {
    if (!UUID.test(raw[key])) throw new Error(`${key} must be a UUID`);
  }
  const { url, insecureLoopback } = normalizeSequencerUrl(raw.SEQUENCER_URL);
  return {
    url,
    insecureLoopback,
    apiKey: raw.SEQUENCER_API_KEY ?? null,
    tenantId: raw.SEQUENCER_TENANT_ID.toLowerCase(),
    storeId: raw.SEQUENCER_STORE_ID.toLowerCase(),
    sequencerPublicKey: raw.SEQUENCER_PUBLIC_KEY
      ? normalizeSequencerPublicKey(raw.SEQUENCER_PUBLIC_KEY)
      : null,
  };
}

/**
 * The config object the CLI's SequencerClient and PeerKeyDirectory read.
 *
 * Deliberately a plain object rather than the CLI's `SyncConfig`: that class
 * resolves credentials through `getApiKey()`, which lets an unrelated
 * `STATESET_API_KEY` in the environment silently replace SEQUENCER_API_KEY.
 * The API key lives in a closure so it never appears in a JSON dump or an
 * inspected object.
 *
 * @param {ReturnType<typeof sequencerSettings>} settings
 * @param {string|null} [agentId]
 */
export function clientConfig(settings, agentId = null) {
  const apiKey = settings.apiKey;
  return {
    sequencerUrl: settings.url,
    tenantId: settings.tenantId,
    storeId: settings.storeId,
    agentId,
    // The board signs plain Ed25519 (VES v1 "legacy" profile). A sequencer
    // running the hybrid or pqc-strict profile refuses these keys.
    securityProfile: 'legacy',
    // Only ever true for http:// to a loopback host (see normalizeSequencerUrl).
    allowInsecureTransport: settings.insecureLoopback,
    sequencerPublicKey: settings.sequencerPublicKey,
    peerKeyTtlSeconds: 300,
    peerKeyMaxStaleSeconds: 86400,
    requestTimeoutMs: 15000,
    batchSize: 100,
    retryPolicy: { maxRetries: 2, baseDelay: 250, maxDelay: 2000 },
    getCredentials: () => ({ apiKey, jwt: null }),
    toJSON: () => ({ sequencerUrl: settings.url, tenantId: settings.tenantId }),
  };
}

/** Whether `child` is inside `parent` (both absolute). */
export function isInside(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}
