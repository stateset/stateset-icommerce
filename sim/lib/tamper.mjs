/**
 * Test helper, run as its own process: apply SQL to a store. It lives in
 * lib/ because `node --test` collects every file under a test/ directory.
 *
 *   node sim/lib/tamper.mjs <db-path> <sql>
 *
 * Its own process for the reason the oracle has one (see runOracleIsolated):
 * a second SQLite copy must never share a process with the engine.
 */
import { createRequire } from 'node:module';
import path from 'node:path';

import { REPO_ROOT } from './engine.mjs';

const [dbPath, sql] = process.argv.slice(2);
let db;
try {
  const Database = createRequire(path.join(REPO_ROOT, 'cli/package.json'))('better-sqlite3');
  db = new Database(dbPath, { fileMustExist: true });
} catch {
  const { DatabaseSync } = await import('node:sqlite');
  db = new DatabaseSync(dbPath);
}
db.exec(sql);
db.close();
