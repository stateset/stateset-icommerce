/**
 * Persistent Session Store for StateSet Channel Gateways
 *
 * SQLite-backed session persistence using better-sqlite3 (already a dependency).
 * Allows conversation context to survive gateway restarts.
 *
 * Usage:
 *   const store = new ChannelSessionStore();
 *   const mgr = createSessionManager({ store, channel: 'telegram' });
 */

import Database from 'better-sqlite3';
import path from 'path';
import os from 'os';
import fs from 'fs';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'channel-sessions.db');

export class ChannelSessionStore {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.dbPath] - Path to SQLite file
   */
  constructor({ dbPath = DEFAULT_DB_PATH } = {}) {
    // Ensure directory exists
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });

    this.db = new Database(dbPath);

    // WAL mode for concurrent access
    this.db.pragma('journal_mode = WAL');

    // Create table
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS channel_sessions (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        channel      TEXT    NOT NULL,
        sender_id    TEXT    NOT NULL,
        session_id   TEXT,
        agent        TEXT,
        last_active  INTEGER NOT NULL DEFAULT (unixepoch() * 1000),
        context      TEXT,
        UNIQUE(channel, sender_id)
      )
    `);

    // Prepared statements
    this._get = this.db.prepare(
      `SELECT session_id, agent, last_active, context
       FROM channel_sessions
       WHERE channel = ? AND sender_id = ?`
    );

    this._upsert = this.db.prepare(
      `INSERT INTO channel_sessions (channel, sender_id, session_id, agent, last_active, context)
       VALUES (?, ?, ?, ?, ?, ?)
       ON CONFLICT(channel, sender_id)
       DO UPDATE SET
         session_id  = excluded.session_id,
         agent       = excluded.agent,
         last_active = excluded.last_active,
         context     = excluded.context`
    );

    this._deleteExpired = this.db.prepare(
      `DELETE FROM channel_sessions WHERE last_active < ?`
    );
  }

  /**
   * Load a session from the database.
   *
   * @param {string} channel - Channel name (e.g. 'telegram')
   * @param {string} senderId - Sender identifier
   * @returns {{ sessionId: string|null, agent: string|null, lastActive: number, context: object|null }|null}
   */
  get(channel, senderId) {
    const row = this._get.get(channel, senderId);
    if (!row) return null;

    let context = null;
    if (row.context) {
      try {
        context = JSON.parse(row.context);
      } catch {
        context = null;
      }
    }

    return {
      sessionId: row.session_id,
      agent: row.agent,
      lastActive: row.last_active,
      context,
    };
  }

  /**
   * Persist a session to the database.
   *
   * @param {string} channel
   * @param {string} senderId
   * @param {Object} session
   * @param {string|null} session.sessionId
   * @param {string|null} session.agent
   * @param {number}      session.lastActive
   * @param {object|null} [session.context]
   */
  upsert(channel, senderId, session) {
    const contextStr = session.context ? JSON.stringify(session.context) : null;
    this._upsert.run(
      channel,
      senderId,
      session.sessionId || null,
      session.agent || null,
      session.lastActive || Date.now(),
      contextStr,
    );
  }

  /**
   * Delete sessions older than the given TTL.
   *
   * @param {number} ttlMs - Max age in milliseconds
   * @returns {number} Number of deleted rows
   */
  deleteExpired(ttlMs) {
    const cutoff = Date.now() - ttlMs;
    const result = this._deleteExpired.run(cutoff);
    return result.changes;
  }

  /**
   * Close the database connection.
   */
  close() {
    this.db.close();
  }
}
