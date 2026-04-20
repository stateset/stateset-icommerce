/**
 * Memory Store for StateSet iCommerce
 *
 * SQLite-backed conversation memory store. Persists summaries, facts, and
 * context from past conversations so agents can recall earlier interactions.
 *
 * Uses better-sqlite3 when available, and falls back to a durable JSON store
 * when the native module is unavailable.
 */

import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import fs, { existsSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';

// ============================================================================
// Schema
// ============================================================================

const SCHEMA = `
CREATE TABLE IF NOT EXISTS conversation_memory (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  channel      TEXT    NOT NULL DEFAULT 'cli',
  sender_id    TEXT    NOT NULL DEFAULT 'local',
  session_id   TEXT,
  summary      TEXT    NOT NULL,
  facts        TEXT,
  agent        TEXT,
  created_at   INTEGER NOT NULL DEFAULT (unixepoch() * 1000),
  token_count  INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_memory_sender
  ON conversation_memory(channel, sender_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_memory_search
  ON conversation_memory(summary);
`;

const require = createRequire(import.meta.url);
const FALLBACK_DATABASES = new Map();
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

function getFallbackDatabasePath(dbPath) {
  return dbPath === ':memory:' ? ':memory:' : `${dbPath}.fallback.json`;
}

function persistFallbackDatabase(state) {
  if (!state?.storagePath || state.storagePath === ':memory:') {
    return;
  }

  mkdirSync(dirname(state.storagePath), { recursive: true });
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
  );
  fs.renameSync(tmpPath, state.storagePath);
}

function getFallbackDatabase(dbPath) {
  const storagePath = getFallbackDatabasePath(dbPath);
  if (storagePath === ':memory:') {
    return { nextId: 1, rows: [], storagePath };
  }

  let state = FALLBACK_DATABASES.get(storagePath);
  if (!state) {
    const rows = [];
    let nextId = 1;
    if (existsSync(storagePath)) {
      try {
        const raw = fs.readFileSync(storagePath, 'utf8').trim();
        if (raw) {
          const parsed = JSON.parse(raw);
          if (Array.isArray(parsed?.rows)) {
            rows.push(...parsed.rows);
          }
          if (Number.isInteger(parsed?.nextId) && parsed.nextId > 0) {
            nextId = parsed.nextId;
          } else if (rows.length > 0) {
            nextId = Math.max(...rows.map((row) => row.id || 0)) + 1;
          }
        }
      } catch (error) {
        console.warn(
          `[memory-store] Failed to read fallback store ${storagePath}: ${error.message}`,
        );
      }
    }
    state = { nextId, rows, storagePath };
    FALLBACK_DATABASES.set(storagePath, state);
  }
  return state;
}

function sortRowsByRecency(rows) {
  return [...rows].sort((a, b) => b.created_at - a.created_at || b.id - a.id);
}

// ============================================================================
// Default path
// ============================================================================

function defaultDbPath() {
  const dir = join(homedir(), '.stateset');
  try {
    mkdirSync(dir, { recursive: true });
  } catch (err) {
    console.debug('[memory-store] Directory creation failed:', err.message || err);
  }
  return join(dir, 'memory.db');
}

// ============================================================================
// MemoryStore
// ============================================================================

export class MemoryStore {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.dbPath] - SQLite database path (default: ~/.stateset/memory.db)
   * @param {typeof import('better-sqlite3') | null} [opts.databaseCtor]
   */
  constructor(opts = {}) {
    this._dbPath = opts.dbPath || defaultDbPath();
    const Database = opts.databaseCtor === undefined ? loadDatabaseCtor() : opts.databaseCtor;
    this._fallbackState = null;
    this.backend = 'sqlite';

    if (!Database) {
      this._enableFallback('better-sqlite3 unavailable');
      return;
    }

    try {
      this._db = new Database(this._dbPath);
    } catch (error) {
      if (error?.code === 'ERR_DLOPEN_FAILED') {
        this._enableFallback(error.message || 'native module load failure');
        return;
      }
      throw error;
    }
    this._db.pragma('journal_mode = WAL');
    this._db.exec(SCHEMA);

    // Prepared statements
    this._insertStmt = this._db.prepare(`
      INSERT INTO conversation_memory (channel, sender_id, session_id, summary, facts, agent, created_at, token_count)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `);

    this._recentStmt = this._db.prepare(`
      SELECT id, channel, sender_id, session_id, summary, facts, agent, created_at, token_count
      FROM conversation_memory
      WHERE channel = ? AND sender_id = ?
      ORDER BY created_at DESC, id DESC
      LIMIT ?
    `);

    this._searchStmt = this._db.prepare(`
      SELECT id, channel, sender_id, session_id, summary, facts, agent, created_at, token_count
      FROM conversation_memory
      WHERE channel = ? AND sender_id = ? AND summary LIKE ?
      ORDER BY created_at DESC, id DESC
      LIMIT ?
    `);

    this._allRecentStmt = this._db.prepare(`
      SELECT id, channel, sender_id, session_id, summary, facts, agent, created_at, token_count
      FROM conversation_memory
      ORDER BY created_at DESC, id DESC
      LIMIT ?
    `);

    this._deleteOldStmt = this._db.prepare(`
      DELETE FROM conversation_memory WHERE created_at < ?
    `);

    this._countStmt = this._db.prepare(`
      SELECT COUNT(*) as cnt FROM conversation_memory
    `);

    this._deleteByIdStmt = this._db.prepare(`
      DELETE FROM conversation_memory WHERE id = ?
    `);

    // Entity search — searches across summary and facts for entity references
    this._entitySearchStmt = this._db.prepare(`
      SELECT id, channel, sender_id, session_id, summary, facts, agent, created_at, token_count
      FROM conversation_memory
      WHERE channel = ? AND sender_id = ?
        AND (summary LIKE ? OR facts LIKE ?)
      ORDER BY created_at DESC, id DESC
      LIMIT ?
    `);
  }

  /**
   * Save a conversation memory.
   * @param {Object} entry
   * @param {string} [entry.channel='cli']
   * @param {string} [entry.senderId='local']
   * @param {string} [entry.sessionId]
   * @param {string} entry.summary
   * @param {string[]} [entry.facts]
   * @param {string} [entry.agent]
   * @param {number} [entry.tokenCount=0]
   * @returns {{ id: number }}
   */
  save({
    channel = 'cli',
    senderId = 'local',
    sessionId = null,
    summary,
    facts = [],
    agent = null,
    tokenCount = 0,
  }) {
    if (this._fallbackState) {
      const row = this._insertFallbackRow({
        channel,
        senderId,
        sessionId,
        summary,
        facts,
        agent,
        createdAt: Date.now(),
        tokenCount,
      });
      persistFallbackDatabase(this._fallbackState);
      return { id: row.id };
    }

    const result = this._insertStmt.run(
      channel,
      senderId,
      sessionId,
      summary,
      JSON.stringify(facts),
      agent,
      Date.now(),
      tokenCount,
    );
    return { id: Number(result.lastInsertRowid) };
  }

  /**
   * Get the most recent memories for a sender.
   * @param {string} channel
   * @param {string} senderId
   * @param {number} [limit=5]
   * @returns {Object[]}
   */
  getRecent(channel = 'cli', senderId = 'local', limit = 5) {
    if (this._fallbackState) {
      return sortRowsByRecency(this._fallbackRows(channel, senderId))
        .slice(0, limit)
        .map((row) => this._deserialize(row));
    }
    return this._recentStmt.all(channel, senderId, limit).map(this._deserialize);
  }

  /**
   * Search memories by text query (LIKE match).
   * @param {string} channel
   * @param {string} senderId
   * @param {string} query
   * @param {number} [limit=5]
   * @returns {Object[]}
   */
  search(channel = 'cli', senderId = 'local', query = '', limit = 5) {
    if (this._fallbackState) {
      return sortRowsByRecency(this._fallbackRows(channel, senderId))
        .filter((row) => row.summary.includes(query))
        .slice(0, limit)
        .map((row) => this._deserialize(row));
    }
    return this._searchStmt.all(channel, senderId, `%${query}%`, limit).map(this._deserialize);
  }

  /**
   * Search memories that mention a specific entity (order, customer, product, return).
   *
   * Performs a LIKE search over the summary and facts columns for the entity ID,
   * so it works with existing stored memories regardless of schema version.
   *
   * @param {string} channel
   * @param {string} senderId
   * @param {string} entityType - 'order' | 'customer' | 'product' | 'return'
   * @param {string} entityId   - The entity identifier (e.g. 'ORD-12345')
   * @param {number} [limit=5]
   * @returns {Object[]}
   */
  searchByEntity(channel = 'cli', senderId = 'local', entityType, entityId, limit = 5) {
    if (!entityId) return [];
    if (this._fallbackState) {
      return sortRowsByRecency(this._fallbackRows(channel, senderId))
        .filter((row) => {
          const facts = row.facts ? JSON.parse(row.facts) : [];
          return (
            row.summary.includes(entityId) || facts.some((fact) => String(fact).includes(entityId))
          );
        })
        .slice(0, limit)
        .map((row) => this._deserialize(row))
        .map((row) => ({ ...row, entityType, entityId }));
    }
    // Escape LIKE wildcards in the entity ID itself so literal % or _ are matched safely
    const escaped = entityId.replace(/[%_\\]/g, (c) => `\\${c}`);
    const pattern = `%${escaped}%`;
    return this._entitySearchStmt
      .all(channel, senderId, pattern, pattern, limit)
      .map(this._deserialize)
      .map((row) => ({ ...row, entityType, entityId }));
  }

  /**
   * Get all recent memories (across all senders).
   * @param {number} [limit=20]
   * @returns {Object[]}
   */
  getAllRecent(limit = 20) {
    if (this._fallbackState) {
      return sortRowsByRecency(this._fallbackState.rows)
        .slice(0, limit)
        .map((row) => this._deserialize(row));
    }
    return this._allRecentStmt.all(limit).map(this._deserialize);
  }

  /**
   * Delete memories older than the given age.
   * @param {number} maxAgeMs - Max age in milliseconds (default: 30 days)
   * @returns {number} - Number of deleted entries
   */
  prune(maxAgeMs = 30 * 24 * 60 * 60 * 1000) {
    const cutoff = Date.now() - maxAgeMs;
    if (this._fallbackState) {
      const before = this._fallbackState.rows.length;
      this._fallbackState.rows = this._fallbackState.rows.filter((row) => row.created_at >= cutoff);
      if (before !== this._fallbackState.rows.length) {
        persistFallbackDatabase(this._fallbackState);
      }
      return before - this._fallbackState.rows.length;
    }
    return this._deleteOldStmt.run(cutoff).changes;
  }

  /**
   * Delete a specific memory by ID.
   * @param {number} id
   * @returns {boolean}
   */
  delete(id) {
    if (this._fallbackState) {
      const numericId = Number(id);
      const index = this._fallbackState.rows.findIndex((row) => row.id === numericId);
      if (index === -1) return false;
      this._fallbackState.rows.splice(index, 1);
      persistFallbackDatabase(this._fallbackState);
      return true;
    }
    return this._deleteByIdStmt.run(id).changes > 0;
  }

  /**
   * Count total memories.
   * @returns {number}
   */
  count() {
    if (this._fallbackState) {
      return this._fallbackState.rows.length;
    }
    return this._countStmt.get().cnt;
  }

  /**
   * Close the database.
   */
  close() {
    if (this._fallbackState) {
      persistFallbackDatabase(this._fallbackState);
    }
    if (this._db) {
      this._db.close();
      this._db = null;
    }
  }

  /** @private */
  _fallbackRows(channel, senderId) {
    return this._fallbackState.rows.filter(
      (row) => row.channel === channel && row.sender_id === senderId,
    );
  }

  /** @private */
  _enableFallback(reason = 'fallback requested') {
    this._fallbackState = getFallbackDatabase(this._dbPath);
    this.backend = 'json-fallback';
    if (this._dbPath !== ':memory:') {
      console.warn(
        `[memory-store] ${reason}; using durable JSON fallback at ${this._fallbackState.storagePath}`,
      );
    }
    this._insertStmt = {
      run: (channel, senderId, sessionId, summary, facts, agent, createdAt, tokenCount) => {
        const row = this._insertFallbackRow({
          channel,
          senderId,
          sessionId,
          summary,
          facts: JSON.parse(facts),
          agent,
          createdAt,
          tokenCount,
        });
        return { lastInsertRowid: row.id };
      },
    };
  }

  /** @private */
  _insertFallbackRow({
    channel,
    senderId,
    sessionId,
    summary,
    facts = [],
    agent,
    createdAt,
    tokenCount,
  }) {
    const row = {
      id: this._fallbackState.nextId++,
      channel,
      sender_id: senderId,
      session_id: sessionId,
      summary,
      facts: JSON.stringify(facts),
      agent,
      created_at: createdAt,
      token_count: tokenCount,
    };
    this._fallbackState.rows.push(row);
    return row;
  }

  /** @private */
  _deserialize(row) {
    return {
      ...row,
      facts: row.facts ? JSON.parse(row.facts) : [],
    };
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global MemoryStore singleton.
 * @param {Object} [opts]
 * @returns {MemoryStore}
 */
export function getMemoryStore(opts) {
  if (!_instance) {
    _instance = new MemoryStore(opts);
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetMemoryStore() {
  if (_instance) {
    _instance.close();
    _instance = null;
  }
}
