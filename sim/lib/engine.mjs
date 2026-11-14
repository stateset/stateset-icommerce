/**
 * Where the harness gets its engine and its independent view of the database.
 *
 * The engine is the in-repo Node binding (`bindings/node/index.js`), so the
 * simulator exercises whatever native binary that binding loads. Set
 * `SIM_BINDING` to point at another `@stateset/embedded` entry point.
 *
 * The oracle reads the SQLite file directly, read-only, instead of asking the
 * engine about itself: better-sqlite3 from the CLI's dependencies when it
 * loads, otherwise Node's built-in `node:sqlite`.
 */
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const SIM_DIR = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
export const REPO_ROOT = path.dirname(SIM_DIR);

const require = createRequire(import.meta.url);

export function bindingPath() {
  return process.env.SIM_BINDING || path.join(REPO_ROOT, 'bindings/node/index.js');
}

/** Load `{ Commerce }`; throws with an actionable message when no native binary is present. */
export function loadBinding() {
  const target = bindingPath();
  try {
    return require(target);
  } catch (error) {
    const wrapped = new Error(
      `cannot load the StateSet Node binding from ${target}: ${error.message}\n` +
        'Build it (npm --prefix bindings/node run build) or set SIM_BINDING to an @stateset/embedded entry point.',
    );
    wrapped.code = 'SIM_BINDING_UNAVAILABLE';
    throw wrapped;
  }
}

export function bindingAvailable() {
  try {
    loadBinding();
    return true;
  } catch {
    return false;
  }
}

/** A minimal read-only `{ all(sql, ...params), get(sql, ...params), close() }`. */
export async function openReader(dbPath) {
  const attempts = [];
  for (const base of [
    path.join(REPO_ROOT, 'cli/package.json'),
    path.join(REPO_ROOT, 'package.json'),
  ]) {
    try {
      const Database = createRequire(base)('better-sqlite3');
      const db = new Database(dbPath, { readonly: true, fileMustExist: true });
      return wrapReader(db, 'better-sqlite3');
    } catch (error) {
      attempts.push(`better-sqlite3 via ${path.relative(REPO_ROOT, base)}: ${error.message}`);
    }
  }
  try {
    const { DatabaseSync } = await import('node:sqlite');
    return wrapReader(new DatabaseSync(dbPath, { readOnly: true }), 'node:sqlite');
  } catch (error) {
    attempts.push(`node:sqlite: ${error.message}`);
  }
  throw new Error(`no SQLite reader available for the oracle:\n  ${attempts.join('\n  ')}`);
}

function wrapReader(db, driver) {
  return {
    driver,
    all: (sql, ...params) => db.prepare(sql).all(...params),
    get: (sql, ...params) => db.prepare(sql).get(...params),
    close: () => db.close(),
  };
}

/** Classify an error thrown (not receipted) by the engine. */
export function classifyThrown(error) {
  const message = String(error?.message ?? error);
  if (/database is locked|database table is locked|SQLITE_BUSY|\bbusy\b/i.test(message))
    return 'busy';
  if (/timed out|timeout/i.test(message)) return 'timeout';
  return 'other';
}
