/**
 * Server-scoped stores for domain tool modules.
 *
 * A tool must read and write the store the MCP server was started with
 * (`--db` / `dbPath`), never a per-machine default such as
 * `~/.stateset/a2a.db`: two servers on one machine would otherwise share
 * those records (a cross-store leak), and a tool's result would depend on the
 * home directory of whoever runs it.
 *
 * The server hands every handler its A2A store as `context.a2aStore` (an
 * `A2AStore` opened on `dbPath`, so the A2A tables live in the commerce
 * database itself). Services built on top of it (checkout links, the agent
 * catalog, circuit breakers, compliance exports) are cached per database so
 * that `stateset-mcp-http`, which builds a fresh server per request, keeps one
 * service -- and one connection -- per store rather than one per request.
 * There is deliberately no home-directory fallback: without a server store
 * the tool refuses.
 */

import path from 'node:path';

import { A2AStore } from '../a2a/store.js';

/** Services keyed by absolute database path, then by service name. */
const servicesByPath = new Map();
/** In-memory stores have no path to share: key their services by the store. */
const servicesByStore = new WeakMap();

/**
 * The A2A store of the server this tool runs in, opened and migrated.
 *
 * @param {object} context - the tool handler context
 * @returns {import('../a2a/store.js').A2AStore}
 */
export function requireScopedA2AStore(context) {
  const store = context?.a2aStore ?? context?.commerce?._store ?? storeForDbPath(context?.dbPath);
  if (!store || typeof store.init !== 'function') {
    throw new Error(
      'This tool needs the server store (context.a2aStore or a file dbPath); it does not fall back to ~/.stateset.',
    );
  }
  store.init();
  return store;
}

/** Stores opened for callers that pass only `dbPath` (e.g. the tool composer). */
const storesByPath = new Map();

/**
 * An A2A store on a commerce database file, one per path. An in-memory
 * database cannot be reopened from its path, so it yields null.
 *
 * @param {string | undefined} dbPath
 * @returns {A2AStore | null}
 */
function storeForDbPath(dbPath) {
  if (!dbPath || isMemoryPath(dbPath)) return null;
  const key = path.resolve(dbPath);
  let store = storesByPath.get(key);
  if (!store || !store.db) {
    store = new A2AStore({ dbPath: key });
    storesByPath.set(key, store);
  }
  return store;
}

function isMemoryPath(dbPath) {
  return dbPath === ':memory:' || String(dbPath).startsWith('file::memory:');
}

function bucketFor(store) {
  const dbPath = store.dbPath;
  if (!dbPath || isMemoryPath(dbPath)) {
    let bucket = servicesByStore.get(store);
    if (!bucket) {
      bucket = new Map();
      servicesByStore.set(store, bucket);
    }
    return bucket;
  }
  const key = path.resolve(dbPath);
  let bucket = servicesByPath.get(key);
  if (!bucket) {
    bucket = new Map();
    servicesByPath.set(key, bucket);
  }
  return bucket;
}

/**
 * One `factory(store)` result per (database, service name).
 *
 * @template T
 * @param {object} context - the tool handler context
 * @param {string} name - service name, unique per tool module
 * @param {(store: import('../a2a/store.js').A2AStore) => T | Promise<T>} factory
 * @returns {Promise<T>}
 */
export async function scopedService(context, name, factory) {
  const store = requireScopedA2AStore(context);
  const bucket = bucketFor(store);
  const cached = bucket.get(name);
  // A service built on a store that has since been closed is dead; rebuild it
  // on the live one.
  if (cached && cached.store.db) return cached.service;
  const entry = { store, service: Promise.resolve().then(() => factory(store)) };
  bucket.set(name, entry);
  try {
    return await entry.service;
  } catch (error) {
    if (bucket.get(name) === entry) bucket.delete(name);
    throw error;
  }
}
