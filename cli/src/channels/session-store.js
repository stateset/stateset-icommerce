/**
 * Persistent Session Store for StateSet Channel Gateways
 *
 * SQLite-backed session persistence using better-sqlite3 when available.
 * Falls back to a durable JSON store when the native SQLite binding is unavailable.
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

function getFallbackSessionPath(dbPath) {
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
        rows: Array.from(state.rows.values()),
      },
      null,
      2,
    ),
  );
  fs.renameSync(tmpPath, state.storagePath);
}

function getFallbackDatabaseState(dbPath) {
  const storagePath = getFallbackSessionPath(dbPath);
  if (storagePath === ':memory:') {
    return { nextId: 1, rows: new Map(), storagePath };
  }

  let state = FALLBACK_SESSION_DATABASES.get(storagePath);
  if (!state) {
    const rows = new Map();
    let nextId = 1;
    if (fs.existsSync(storagePath)) {
      try {
        const raw = fs.readFileSync(storagePath, 'utf8').trim();
        if (raw) {
          const parsed = JSON.parse(raw);
          for (const row of parsed?.rows || []) {
            if (!row?.channel || !row?.sender_id) continue;
            rows.set(fallbackSessionKey(row.channel, row.sender_id), row);
          }
          if (Number.isInteger(parsed?.nextId) && parsed.nextId > 0) {
            nextId = parsed.nextId;
          } else if (rows.size > 0) {
            nextId = Math.max(...Array.from(rows.values(), (row) => row.id || 0)) + 1;
          }
        }
      } catch (error) {
        console.warn(
          `[channel-session-store] Failed to read fallback store ${storagePath}: ${error.message}`,
        );
      }
    }
    state = { nextId, rows, storagePath };
    FALLBACK_SESSION_DATABASES.set(storagePath, state);
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
            const [channel, senderId, sessionId, agent, lastActive, context] = params;
            const key = fallbackSessionKey(channel, senderId);
            const existing = state.rows.get(key);
            state.rows.set(key, {
              id: existing?.id ?? state.nextId++,
              channel,
              sender_id: senderId,
              session_id: sessionId ?? null,
              agent: agent ?? null,
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
   * @param {typeof import('better-sqlite3') | null} [opts.databaseCtor]
   */
  constructor({ dbPath = DEFAULT_DB_PATH, databaseCtor } = {}) {
    this._dbPath = dbPath;
    this._fallbackState = null;
    this._databaseCtor = databaseCtor;
    this.backend = 'sqlite';
    ensureDbFile(dbPath);

    const Database = databaseCtor === undefined ? loadDatabaseCtor() : databaseCtor;
    if (!Database) {
      this._enableFallback('better-sqlite3 unavailable');
      return;
    }

    try {
      this.db = new Database(dbPath);
    } catch (error) {
      if (error?.code !== 'ERR_DLOPEN_FAILED') {
        throw error;
      }
      this._enableFallback(error.message || 'native module load failure');
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

  _enableFallback(reason = 'fallback requested') {
    this._fallbackState = getFallbackDatabaseState(this._dbPath);
    this.backend = 'json-fallback';
    if (this._dbPath !== ':memory:') {
      console.warn(
        `[channel-session-store] ${reason}; using durable JSON fallback at ${this._fallbackState.storagePath}`,
      );
    }
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
      persistFallbackDatabaseState(this._fallbackState);
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
      if (deleted > 0) {
        persistFallbackDatabaseState(this._fallbackState);
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
    if (this._fallbackState) {
      persistFallbackDatabaseState(this._fallbackState);
    }
    this.db.close();
  }
}
