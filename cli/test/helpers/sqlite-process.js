import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const worker = fileURLToPath(new URL('./sqlite-process-worker.mjs', import.meta.url));

/**
 * Inspect or fault-inject a native store from a separate process. Loading a
 * second bundled SQLite in the engine process gives it independent lock/WAL
 * bookkeeping; closing either driver can invalidate the other's file handles.
 * Each batch runs in one transaction and closes before this function returns.
 */
export function sqliteOperations(dbPath, operations) {
  const result = spawnSync(process.execPath, [worker, dbPath], {
    input: JSON.stringify(operations),
    encoding: 'utf8',
    timeout: 30_000,
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`SQLite inspection failed: ${result.stderr || result.signal || result.status}`);
  }
  return JSON.parse(result.stdout);
}

export function sqliteGet(dbPath, sql, ...params) {
  return sqliteOperations(dbPath, [{ mode: 'get', sql, params }])[0];
}

export function sqliteExec(dbPath, sql) {
  sqliteOperations(dbPath, [{ mode: 'exec', sql }]);
}
