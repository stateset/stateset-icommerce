/**
 * `node --import` preload that reports every better-sqlite3 connection the
 * process opens or closes, as one stderr line each:
 *
 *   [side-store-probe] {"event":"open","file":"/abs/store.db"}
 *
 * It wraps the module export in the CommonJS cache before any application
 * code loads, so it sees every in-process user (A2AStore, scoped stores, and
 * anything else) without changing them. Lines are written synchronously so
 * they survive `process.exit()`.
 */

import { writeSync } from 'node:fs';
import { createRequire } from 'node:module';

// Matched by test/integration/mcp-http-side-store-lifetime.test.js.
const PROBE_TAG = '[side-store-probe] ';

const require = createRequire(import.meta.url);
const id = require.resolve('better-sqlite3');
const Database = require(id);

const emit = (event, file) => {
  writeSync(2, `${PROBE_TAG}${JSON.stringify({ event, file: String(file) })}\n`);
};

function ProbedDatabase(filename, options) {
  const db = new Database(filename, options);
  emit('open', db.name);
  return db;
}
ProbedDatabase.prototype = Database.prototype;
Object.setPrototypeOf(ProbedDatabase, Database);

const close = Database.prototype.close;
Database.prototype.close = function probedClose(...args) {
  const wasOpen = this.open;
  const result = close.apply(this, args);
  if (wasOpen) emit('close', this.name);
  return result;
};

require.cache[id].exports = ProbedDatabase;
