/**
 * Persistent Audit Log Store (SQLite-backed)
 *
 * Provides a durable, queryable audit log for all permission checks
 * and tool executions. Survives process restarts and supports compliance exports.
 */

import { createRequire } from 'node:module';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'audit.db');
const require = createRequire(import.meta.url);
const FALLBACK_AUDIT_DATABASES = new Map();
let cachedDatabaseCtor;

/** @type {AuditStore | null} */
let _instance = null;

function loadDatabaseCtor() {
  if (cachedDatabaseCtor !== undefined) {
    return cachedDatabaseCtor;
  }

  try {
    const mod = require('better-sqlite3');
    cachedDatabaseCtor = mod.default || mod;
  } catch (error) {
    if (error?.code !== 'ERR_DLOPEN_FAILED' && error?.code !== 'MODULE_NOT_FOUND') {
      throw error;
    }
    cachedDatabaseCtor = null;
  }

  return cachedDatabaseCtor;
}

function ensureDbFile(dbPath) {
  if (dbPath === ':memory:') return;
  fs.mkdirSync(path.dirname(dbPath), { recursive: true });
  const fd = fs.openSync(dbPath, 'a');
  fs.closeSync(fd);
}

function getFallbackAuditPath(dbPath) {
  return dbPath === ':memory:' ? ':memory:' : `${dbPath}.fallback.json`;
}

function persistFallbackDatabaseState(state) {
  if (!state?.storagePath || state.storagePath === ':memory:') {
    return;
  }

  fs.mkdirSync(path.dirname(state.storagePath), { recursive: true });
  const tmpPath = `${state.storagePath}.tmp`;
  fs.writeFileSync(
    tmpPath,
    JSON.stringify(
      {
        nextId: state.nextId,
        rows: state.rows,
      },
      null,
      2,
    ),
    { mode: 0o600 },
  );
  fs.renameSync(tmpPath, state.storagePath);
  try {
    fs.chmodSync(state.storagePath, 0o600);
  } catch {
    // Best-effort permission hardening; some filesystems do not support chmod.
  }
}

function getFallbackDatabaseState(dbPath) {
  const storagePath = getFallbackAuditPath(dbPath);
  if (storagePath === ':memory:') {
    return { nextId: 1, rows: [], storagePath };
  }

  let state = FALLBACK_AUDIT_DATABASES.get(storagePath);
  if (!state) {
    let rows = [];
    let nextId = 1;

    if (fs.existsSync(storagePath)) {
      try {
        const raw = fs.readFileSync(storagePath, 'utf-8').trim();
        if (raw) {
          const parsed = JSON.parse(raw);
          if (Array.isArray(parsed?.rows)) {
            rows = parsed.rows;
          }
          if (Number.isInteger(parsed?.nextId) && parsed.nextId > 0) {
            nextId = parsed.nextId;
          } else if (rows.length > 0) {
            nextId = Math.max(...rows.map((row) => row.id || 0)) + 1;
          }
        }
      } catch (error) {
        console.warn(
          `[audit-store] Failed to read fallback audit log ${storagePath}: ${error.message}`,
        );
      }
    }

    state = { nextId, rows, storagePath };
    FALLBACK_AUDIT_DATABASES.set(storagePath, state);
  }
  return state;
}

function sortRowsByTimestamp(rows) {
  return [...rows].sort(
    (a, b) => b.timestamp.localeCompare(a.timestamp) || b.id - a.id,
  );
}

export class AuditStore {
  /**
   * @param {object} [options]
   * @param {string} [options.dbPath] - Path to audit SQLite database
   * @param {number} [options.maxEntries] - Max entries to keep (0 = unlimited)
   * @param {number} [options.retentionDays] - Days to keep entries (default: 90)
   * @param {typeof import('better-sqlite3') | null} [options.databaseCtor] - Override database constructor for tests
   */
  constructor({ dbPath = DEFAULT_DB_PATH, maxEntries = 0, retentionDays = 90, databaseCtor } = {}) {
    if (dbPath !== ':memory:') {
      fs.mkdirSync(path.dirname(dbPath), { recursive: true });
    }
    ensureDbFile(dbPath);

    this.maxEntries = maxEntries;
    this.retentionDays = retentionDays;
    this._fallbackState = null;
    this.backend = 'sqlite';

    const Database = databaseCtor === undefined ? loadDatabaseCtor() : databaseCtor;
    if (!Database) {
      this._enableFallback(dbPath, 'better-sqlite3 unavailable');
      return;
    }

    try {
      this.db = new Database(dbPath);
    } catch (error) {
      if (error?.code !== 'ERR_DLOPEN_FAILED') {
        throw error;
      }
      this._enableFallback(dbPath, error.message || 'native module load failure');
      return;
    }
    this.db.pragma('journal_mode = WAL');

    this.db.exec(`
      CREATE TABLE IF NOT EXISTS audit_log (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp   TEXT NOT NULL,
        tool        TEXT NOT NULL,
        params      TEXT,
        result      TEXT NOT NULL,
        reason      TEXT,
        level       TEXT NOT NULL,
        session_id  TEXT,
        agent       TEXT
      );

      CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp DESC);
      CREATE INDEX IF NOT EXISTS idx_audit_tool ON audit_log(tool);
      CREATE INDEX IF NOT EXISTS idx_audit_result ON audit_log(result);
    `);

    this._insert = this.db.prepare(`
      INSERT INTO audit_log (timestamp, tool, params, result, reason, level, session_id, agent)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `);

    this._query = this.db.prepare(`
      SELECT * FROM audit_log
      WHERE (@tool IS NULL OR tool = @tool)
        AND (@result IS NULL OR result = @result)
        AND (@since IS NULL OR timestamp >= @since)
      ORDER BY timestamp DESC
      LIMIT @limit
    `);

    this._count = this.db.prepare(`SELECT COUNT(*) as count FROM audit_log`);

    this._cleanup = this.db.prepare(`
      DELETE FROM audit_log WHERE timestamp < ?
    `);
  }

  _enableFallback(dbPath, reason = 'fallback requested') {
    this._fallbackState = getFallbackDatabaseState(dbPath);
    this.backend = 'json-fallback';
    if (dbPath !== ':memory:') {
      console.warn(
        `[audit-store] ${reason}; using durable JSON fallback at ${this._fallbackState.storagePath}`,
      );
    }
    this.db = {
      pragma() {
        return 'WAL';
      },
      exec() {
        return this;
      },
      close() {},
    };
  }

  /**
   * Log an audit entry.
   * @param {object} entry
   * @param {string} entry.tool - Tool name
   * @param {object} [entry.params] - Sanitized parameters
   * @param {string} entry.result - 'allowed', 'denied', 'executed'
   * @param {string} [entry.reason] - Reason for denial
   * @param {string} entry.level - Permission level
   * @param {string} [entry.sessionId] - Session identifier
   * @param {string} [entry.agent] - Agent name
   */
  log(entry) {
    if (this._fallbackState) {
      this._fallbackState.rows.push({
        id: this._fallbackState.nextId++,
        timestamp: new Date().toISOString(),
        tool: entry.tool,
        params: entry.params ? JSON.stringify(entry.params) : null,
        result: entry.result,
        reason: entry.reason || null,
        level: entry.level,
        session_id: entry.sessionId || null,
        agent: entry.agent || null,
      });

      if (this.maxEntries > 0 && this._fallbackState.rows.length > this.maxEntries) {
        this._fallbackState.rows = sortRowsByTimestamp(this._fallbackState.rows).slice(
          0,
          this.maxEntries,
        );
      }
      persistFallbackDatabaseState(this._fallbackState);
      return;
    }

    this._insert.run(
      new Date().toISOString(),
      entry.tool,
      entry.params ? JSON.stringify(entry.params) : null,
      entry.result,
      entry.reason || null,
      entry.level,
      entry.sessionId || null,
      entry.agent || null,
    );
  }

  /**
   * Query audit entries.
   * @param {object} [options]
   * @param {string} [options.tool] - Filter by tool name
   * @param {string} [options.result] - Filter by result
   * @param {string} [options.since] - ISO timestamp to filter from
   * @param {number} [options.limit] - Max entries to return (default: 100)
   * @returns {Array<object>}
   */
  query({ tool = null, result = null, since = null, limit = 100 } = {}) {
    if (this._fallbackState) {
      return sortRowsByTimestamp(this._fallbackState.rows)
        .filter(
          (row) =>
            (tool === null || row.tool === tool) &&
            (result === null || row.result === result) &&
            (since === null || row.timestamp >= since),
        )
        .slice(0, limit)
        .map((row) => ({
          ...row,
          params: row.params ? JSON.parse(row.params) : null,
        }));
    }

    const rows = this._query.all({ tool, result, since, limit });
    return rows.map((row) => ({
      ...row,
      params: row.params ? JSON.parse(row.params) : null,
    }));
  }

  /**
   * Get total count of audit entries.
   * @returns {number}
   */
  count() {
    if (this._fallbackState) {
      return this._fallbackState.rows.length;
    }
    return this._count.get().count;
  }

  /**
   * Remove entries older than retention period.
   */
  cleanup() {
    const cutoff = new Date();
    cutoff.setDate(cutoff.getDate() - this.retentionDays);
    if (this._fallbackState) {
      const cutoffIso = cutoff.toISOString();
      this._fallbackState.rows = this._fallbackState.rows.filter(
        (row) => row.timestamp >= cutoffIso,
      );
      persistFallbackDatabaseState(this._fallbackState);
      return;
    }
    this._cleanup.run(cutoff.toISOString());
  }

  /**
   * Export the audit log for compliance.
   * @param {object} [options]
   * @param {string} [options.since] - ISO timestamp
   * @param {number} [options.limit] - Max entries (default: 10000)
   * @returns {object}
   */
  export({ since = null, limit = 10000 } = {}) {
    return {
      exportedAt: new Date().toISOString(),
      totalEntries: this.count(),
      entries: this.query({ since, limit }),
    };
  }

  /**
   * Close the database connection.
   */
  close() {
    if (this._fallbackState) {
      persistFallbackDatabaseState(this._fallbackState);
    }
    this.db.close();
  }
}

/**
 * Get the singleton AuditStore instance.
 * @param {object} [options]
 * @returns {AuditStore}
 */
export function getAuditStore(options) {
  if (!_instance) {
    _instance = new AuditStore(options);
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetAuditStore() {
  if (_instance) {
    _instance.close();
    _instance = null;
  }
}
