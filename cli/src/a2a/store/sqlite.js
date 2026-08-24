/**
 * A2A Store — better-sqlite3 loader with cached load failure.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
let cachedDatabaseCtor;
let cachedDatabaseLoadError = null;

/**
 * Lazily require better-sqlite3, caching both the constructor and any load error.
 * @returns {any} The Database constructor, or `null` when the native module is unavailable.
 */
export function loadDatabaseCtor() {
  if (cachedDatabaseCtor !== undefined) {
    return cachedDatabaseCtor;
  }

  try {
    const mod = require('better-sqlite3');
    cachedDatabaseCtor = mod.default || mod;
    cachedDatabaseLoadError = null;
  } catch (error) {
    if (error?.code !== 'ERR_DLOPEN_FAILED' && error?.code !== 'MODULE_NOT_FOUND') {
      throw error;
    }
    cachedDatabaseCtor = null;
    cachedDatabaseLoadError = error;
  }

  return cachedDatabaseCtor;
}

/**
 * Build the A2A_STORE_SQLITE_UNAVAILABLE error surfaced when better-sqlite3 cannot load.
 * @param {unknown} [error] Underlying load error (defaults to the cached one).
 * @returns {Error & { code: string, cause?: unknown }}
 */
export function createSqliteUnavailableError(error = cachedDatabaseLoadError) {
  const wrapped = new Error(
    'better-sqlite3 is required to use A2AStore. Rebuild it with `npm --prefix cli rebuild better-sqlite3`.',
  );
  wrapped.code = 'A2A_STORE_SQLITE_UNAVAILABLE';
  if (error) {
    wrapped.cause = error;
  }
  return wrapped;
}
