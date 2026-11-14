/**
 * ERC-8004 Identity Registry helpers (SQLite-backed)
 *
 * For a file path, each call runs in a short-lived child process that opens
 * the database, performs one operation and closes it (the arrangement of
 * ../compliance/isolated.js). The path is usually the native engine's own
 * store. POSIX locks belong to a process, so a second SQLite copy inside the
 * engine's process cannot see the engine's locks: opening and closing it
 * dropped them, and its close then checkpointed and unlinked the engine's
 * -wal/-shm, so later engine commits went to an unlinked WAL. A separate
 * process does see the engine's locks; its close leaves the WAL alone.
 *
 * The calls stay synchronous (spawnSync), which blocks this thread for the
 * child's lifetime. `:memory:` has no file to share and runs in-process.
 */

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { deserialize, serialize } from 'node:v8';
import * as store from './store.js';

const workerPath = fileURLToPath(new URL('./worker.js', import.meta.url));
const WORKER_TIMEOUT_MS = 60_000;
const WRITE_OPERATIONS = new Set(['registerIdentity', 'setAgentWallet']);
const ERROR_TYPES = { TypeError, RangeError, SyntaxError };

const REGISTER_FIELDS = [
  'id',
  'agentRegistry',
  'agentId',
  'agentUri',
  'agentWallet',
  'ownerAddress',
  'agentCardId',
  'registration',
  'registrationHash',
  'walletProofType',
  'walletProof',
  'walletProofChainId',
  'walletProofDeadline',
  'active',
];
const WALLET_FIELDS = [
  'agentRegistry',
  'agentId',
  'agentWallet',
  'walletProofType',
  'walletProof',
  'walletProofChainId',
  'walletProofDeadline',
];
const LIST_FIELDS = ['agentRegistry', 'agentId', 'agentWallet', 'active', 'limit'];

/**
 * Copy the fields the store reads, by property access (so getters and
 * inherited properties behave as before). Non-objects pass through so the
 * store raises the same error it always did.
 */
function pick(value, fields) {
  if (value === null || typeof value !== 'object') return value;
  const picked = {};
  for (const field of fields) {
    if (value[field] !== undefined) picked[field] = value[field];
  }
  return picked;
}

/** The worker can only honour the `databaseCtor: null` (JSON fallback) seam. */
function workerOptions(options) {
  if (options === null || typeof options !== 'object') return options;
  if (options.databaseCtor === undefined) return {};
  if (options.databaseCtor === null) return { databaseCtor: null };
  throw new TypeError(
    'options.databaseCtor must be null or undefined for a file-backed ERC-8004 store; ' +
      'SQLite runs in a separate process',
  );
}

function restoreError(info) {
  const ErrorType = ERROR_TYPES[info?.name] || Error;
  const error = new ErrorType(info?.message ?? 'ERC-8004 operation failed');
  if (info?.name && error.name !== info.name) error.name = info.name;
  if (info?.code !== undefined) error.code = info.code;
  return error;
}

function runInWorker(operation, args) {
  const result = spawnSync(process.execPath, [workerPath], {
    input: serialize({ operation, args }),
    timeout: WORKER_TIMEOUT_MS,
    maxBuffer: Infinity,
    windowsHide: true,
  });
  const stderr = result.stderr ? result.stderr.toString('utf8').trimEnd() : '';
  let reply = null;
  if (!result.error && result.status === 0) {
    try {
      reply = deserialize(result.stdout);
    } catch {
      reply = null;
    }
  }
  if (!reply || typeof reply.ok !== 'boolean') {
    const write = WRITE_OPERATIONS.has(operation);
    const failure = new Error(
      write
        ? 'ERC-8004 worker failed; the identity write may or may not have been applied.'
        : 'ERC-8004 worker failed before returning a complete result.',
    );
    failure.code = write ? 'ERC8004_OUTCOME_UNKNOWN' : 'ERC8004_WORKER_FAILED';
    failure.cause =
      result.error ||
      new Error(
        `worker exited with ${result.signal || `status ${result.status}`}${stderr ? `: ${stderr}` : ''}`,
      );
    throw failure;
  }
  // The fallback store reports itself on stderr; surface it as before.
  if (stderr) console.warn(stderr);
  if (!reply.ok) throw restoreError(reply.error);
  return reply.result;
}

function dispatch(operation, dbPath, args, options) {
  if (dbPath === ':memory:') {
    return store[operation](dbPath, ...args, options);
  }
  return runInWorker(operation, [dbPath, ...args, workerOptions(options)]);
}

export function registerIdentity(dbPath, input, options = {}) {
  return dispatch('registerIdentity', dbPath, [pick(input, REGISTER_FIELDS)], options);
}

export function setAgentWallet(dbPath, input, options = {}) {
  return dispatch('setAgentWallet', dbPath, [pick(input, WALLET_FIELDS)], options);
}

export function getIdentity(dbPath, agentRegistry, agentId, options = {}) {
  return dispatch('getIdentity', dbPath, [agentRegistry, agentId], options);
}

export function getIdentityByWallet(dbPath, wallet, options = {}) {
  return dispatch('getIdentityByWallet', dbPath, [wallet], options);
}

export function listIdentities(dbPath, filter = {}, options = {}) {
  return dispatch('listIdentities', dbPath, [pick(filter, LIST_FIELDS)], options);
}

export default {
  registerIdentity,
  setAgentWallet,
  getIdentity,
  getIdentityByWallet,
  listIdentities,
};
