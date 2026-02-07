/**
 * Persistent Audit Log Store (SQLite-backed)
 *
 * Provides a durable, queryable audit log for all permission checks
 * and tool executions. Survives process restarts and supports compliance exports.
 */

import Database from 'better-sqlite3';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'audit.db');

/** @type {AuditStore | null} */
let _instance = null;

export class AuditStore {
  /**
   * @param {object} [options]
   * @param {string} [options.dbPath] - Path to audit SQLite database
   * @param {number} [options.maxEntries] - Max entries to keep (0 = unlimited)
   * @param {number} [options.retentionDays] - Days to keep entries (default: 90)
   */
  constructor({ dbPath = DEFAULT_DB_PATH, maxEntries = 0, retentionDays = 90 } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });

    this.db = new Database(dbPath);
    this.db.pragma('journal_mode = WAL');
    this.maxEntries = maxEntries;
    this.retentionDays = retentionDays;

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
    return this._count.get().count;
  }

  /**
   * Remove entries older than retention period.
   */
  cleanup() {
    const cutoff = new Date();
    cutoff.setDate(cutoff.getDate() - this.retentionDays);
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
