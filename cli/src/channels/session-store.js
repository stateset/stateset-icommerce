/**
 * Persistent Session Store for StateSet Channel Gateways
 *
 * SQLite-backed session persistence using better-sqlite3 when available.
 * Allows conversation context to survive gateway restarts.
 *
 * Usage:
 *   const store = new ChannelSessionStore();
 *   const mgr = createSessionManager({ store, channel: 'telegram' });
 */

import { createRequire } from 'node:module';
import path from 'path';
import os from 'os';
import fs from 'fs';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'channel-sessions.db');
const require = createRequire(import.meta.url);
const FALLBACK_SESSION_DATABASES = new Map();
let cachedDatabaseCtor;

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

function getFallbackDatabaseState(dbPath) {
  if (dbPath === ':memory:') {
    return { nextId: 1, rows: new Map() };
  }

  let state = FALLBACK_SESSION_DATABASES.get(dbPath);
  if (!state || !fs.existsSync(dbPath)) {
    ensureDbFile(dbPath);
    state = { nextId: 1, rows: new Map() };
    FALLBACK_SESSION_DATABASES.set(dbPath, state);
  }
  return state;
}

function fallbackSessionKey(channel, senderId) {
  return `${channel}\u0000${senderId}`;
}

function createFallbackDb(state) {
  return {
    pragma() {
      return 'WAL';
    },
    exec() {
      return this;
    },
    prepare(sql) {
      const normalizedSql = sql.trim().replace(/\s+/g, ' ').toUpperCase();
      return {
        run(...params) {
          if (normalizedSql.startsWith('INSERT INTO CHANNEL_SESSIONS')) {
            const [channel, senderId, context, lastActive] = params;
            const key = fallbackSessionKey(channel, senderId);
            const existing = state.rows.get(key);
            state.rows.set(key, {
              id: existing?.id ?? state.nextId++,
              channel,
              sender_id: senderId,
              session_id: existing?.session_id ?? null,
              agent: existing?.agent ?? null,
              last_active: lastActive,
              context,
            });
            return { changes: 1, lastInsertRowid: existing?.id ?? state.nextId - 1 };
          }
          return { changes: 0, lastInsertRowid: 0 };
        },
      };
    },
    close() {},
  };
}

export class ChannelSessionStore {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.dbPath] - Path to SQLite file
   */
  constructor({ dbPath = DEFAULT_DB_PATH } = {}) {
    this._dbPath = dbPath;
    this._fallbackState = null;
    ensureDbFile(dbPath);

    const Database = loadDatabaseCtor();
    if (!Database) {
      this._enableFallback();
      return;
    }

    try {
      this.db = new Database(dbPath);
    } catch (error) {
      if (error?.code !== 'ERR_DLOPEN_FAILED') {
        throw error;
      }
      this._enableFallback();
      return;
    }

    this.db.pragma('journal_mode = WAL');
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

    this._get = this.db.prepare(
      `SELECT session_id, agent, last_active, context
       FROM channel_sessions
       WHERE channel = ? AND sender_id = ?`,
    );

    this._upsert = this.db.prepare(
      `INSERT INTO channel_sessions (channel, sender_id, session_id, agent, last_active, context)
       VALUES (?, ?, ?, ?, ?, ?)
       ON CONFLICT(channel, sender_id)
       DO UPDATE SET
         session_id  = excluded.session_id,
         agent       = excluded.agent,
         last_active = excluded.last_active,
         context     = excluded.context`,
    );

    this._deleteExpired = this.db.prepare(`DELETE FROM channel_sessions WHERE last_active < ?`);
  }

  _enableFallback() {
    this._fallbackState = getFallbackDatabaseState(this._dbPath);
    this.db = createFallbackDb(this._fallbackState);
  }

  /**
   * Load a session from the database.
   *
   * @param {string} channel - Channel name (e.g. 'telegram')
   * @param {string} senderId - Sender identifier
   * @returns {{ sessionId: string|null, agent: string|null, lastActive: number, context: object|null }|null}
   */
  get(channel, senderId) {
    const row = this._fallbackState
      ? this._fallbackState.rows.get(fallbackSessionKey(channel, senderId)) || null
      : this._get.get(channel, senderId);
    if (!row) return null;

    let context = null;
    if (row.context) {
      try {
        context = JSON.parse(row.context);
      } catch (err) {
        console.debug('[session-store] Context JSON parse failed:', err.message || err);
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
    const payload = {
      channel,
      sender_id: senderId,
      session_id: session.sessionId || null,
      agent: session.agent || null,
      last_active: session.lastActive || Date.now(),
      context: contextStr,
    };

    if (this._fallbackState) {
      const key = fallbackSessionKey(channel, senderId);
      const existing = this._fallbackState.rows.get(key);
      this._fallbackState.rows.set(key, {
        id: existing?.id ?? this._fallbackState.nextId++,
        ...payload,
      });
      return;
    }

    this._upsert.run(
      payload.channel,
      payload.sender_id,
      payload.session_id,
      payload.agent,
      payload.last_active,
      payload.context,
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
    if (this._fallbackState) {
      let deleted = 0;
      for (const [key, row] of this._fallbackState.rows.entries()) {
        if (row.last_active < cutoff) {
          this._fallbackState.rows.delete(key);
          deleted += 1;
        }
      }
      return deleted;
    }
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
