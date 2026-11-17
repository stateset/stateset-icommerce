/**
 * The SQLite handle `stateset-sync` runs the VES outbox on.
 *
 * The outbox tables live in the engine's store file (`./store.db` by default).
 * When the native engine (`@stateset/embedded`) can be loaded, the store is
 * opened through it and the outbox runs on the engine's own side store
 * (`commerce.openSideStore()`), so exactly one SQLite library owns the file in
 * this process -- the same arrangement every other host uses. Only when the
 * engine cannot be loaded (no prebuilt binary for this platform, or one too
 * old to provide side stores) does it fall back to `better-sqlite3`, which is
 * then the only SQLite library with the file open here.
 */

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

/**
 * A better-sqlite3-shaped handle over an engine's side store that also owns
 * the engine: `close()` closes both. `closed` settles once the engine has
 * released the file.
 */
export class EngineSyncDatabase {
  /**
   * @param {any} engine - the native `Commerce`
   * @param {any} side - `engine.openSideStore()`
   */
  constructor(engine, side) {
    this.engine = engine;
    this.side = side;
    /** @type {Promise<void> | null} */
    this.closed = null;
  }

  /** @param {string} sql */
  prepare(sql) {
    return this.side.prepare(sql);
  }

  /** @param {string} sql */
  exec(sql) {
    this.side.exec(sql);
    return this;
  }

  /**
   * @param {string} source
   * @param {{ simple?: boolean }} [options]
   */
  pragma(source, options) {
    return this.side.pragma(source, options);
  }

  /** @param {(...args: any[]) => any} fn */
  transaction(fn) {
    return this.side.transaction(fn);
  }

  get inTransaction() {
    return this.side.inTransaction;
  }

  get open() {
    return this.side.open;
  }

  /** Close the side store and the engine. Idempotent; see `closed`. */
  close() {
    if (!this.closed) {
      this.side.close();
      this.closed = Promise.resolve(this.engine.close());
    }
    return this;
  }
}

/** The native `Commerce` constructor, or null when it cannot be loaded. */
function loadEngineCtor() {
  try {
    const mod = require('@stateset/embedded');
    return mod.Commerce ?? null;
  } catch {
    return null;
  }
}

async function loadBetterSqlite3() {
  const mod = await import('better-sqlite3');
  return mod.default ?? mod;
}

/**
 * Open `dbPath` for the sync outbox: through the engine's side store when the
 * engine is available, otherwise through better-sqlite3.
 *
 * @param {string} dbPath
 * @param {{ loadEngine?: () => any, loadDatabase?: () => Promise<any> }} [loaders] - test seams
 * @returns {Promise<any>} a better-sqlite3-compatible handle; `close()` releases everything
 */
export async function openSyncDatabase(dbPath, loaders = {}) {
  const Commerce = (loaders.loadEngine ?? loadEngineCtor)();
  if (Commerce && typeof Commerce.prototype?.openSideStore === 'function') {
    const engine = await Commerce.open(dbPath);
    try {
      return new EngineSyncDatabase(engine, engine.openSideStore());
    } catch (error) {
      await engine.close();
      throw error;
    }
  }
  const Database = await (loaders.loadDatabase ?? loadBetterSqlite3)();
  return new Database(dbPath);
}
