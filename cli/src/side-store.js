/**
 * The engine's own SQLite handle for side tables.
 *
 * Several CLI stores keep auxiliary tables in the engine's store file: the
 * VES sync outbox, the import id map, the marketplace bridge inbox, the
 * commerce half of compliance exports. They used to reach that file through
 * `better-sqlite3`, which bundles its own copy of SQLite. The native engine
 * links another, and POSIX locks belong to the process while each SQLite copy
 * keeps its own lock bookkeeping, so closing the better-sqlite3 connection
 * dropped locks the engine still relied on and unlinked its WAL. Later engine
 * commits vanished and the file eventually reported "database disk image is
 * malformed".
 *
 * `@stateset/embedded` now hands out a side store (`commerce.openSideStore()`):
 * a synchronous handle opened by the engine's own SQLite on the engine's own
 * file (its private temp file for `:memory:`), speaking the subset of the
 * better-sqlite3 `Database` API those stores use. Whenever a store is given an
 * engine it must take its handle from here, never open the file itself.
 *
 * One side store is shared per engine (`commerce.sharedSideStore()`): every
 * store on that engine then uses the same connection, so a transaction one
 * store opens can never wait on a write lock another store's connection
 * holds. It is keyed by the native engine, not by the wrapper a caller holds:
 * the MCP server builds fresh wrappers around one engine for every request,
 * and a store per wrapper would leak a connection per request. The engine
 * owns it -- `Commerce.close()` closes it -- so callers must not close it.
 */

/**
 * Fallback cache for engine objects that offer only `openSideStore()`.
 * @type {WeakMap<object, any>}
 */
const sharedByEngine = new WeakMap();

/**
 * Whether `commerce` can hand out a side store (a native `Commerce`, or an
 * adapter over one).
 *
 * @param {unknown} commerce
 * @returns {boolean}
 */
export function supportsSideStore(commerce) {
  return (
    commerce !== null &&
    (typeof commerce === 'object' || typeof commerce === 'function') &&
    typeof (/** @type {any} */ (commerce).openSideStore) === 'function'
  );
}

/**
 * The shared side store of `commerce`, opened on first use; `null` when the
 * engine cannot provide one. Do not close it: the engine's `close()` does.
 * Throws the engine's `PRECONDITION_FAILED` error once the engine is closed.
 *
 * @param {unknown} commerce
 * @returns {any | null} a better-sqlite3-compatible handle
 */
export function engineSideStore(commerce) {
  if (!supportsSideStore(commerce)) return null;
  const engine = /** @type {any} */ (commerce);
  if (typeof engine.sharedSideStore === 'function') return engine.sharedSideStore();
  let store = sharedByEngine.get(engine);
  if (!store || !store.open) {
    store = engine.openSideStore();
    sharedByEngine.set(engine, store);
  }
  return store;
}

/**
 * The better-sqlite3-style handle a side-table store should run on:
 *
 * 1. an explicit `db` (caller-owned, e.g. a standalone file with no engine);
 * 2. `commerce.db`, for engine objects that expose one;
 * 3. the engine's shared side store.
 *
 * Returns `null` when none is available.
 *
 * @param {{ db?: any, commerce?: any }} [sources]
 * @returns {any | null}
 */
export function resolveSideDb({ db, commerce } = {}) {
  if (db && typeof db.prepare === 'function') return db;
  const legacy = commerce?.db;
  if (legacy && typeof legacy.prepare === 'function') return legacy;
  return engineSideStore(commerce);
}
