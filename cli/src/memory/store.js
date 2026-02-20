/**
 * Memory Store for StateSet iCommerce
 *
 * SQLite-backed conversation memory store. Persists summaries, facts, and
 * context from past conversations so agents can recall earlier interactions.
 *
 * Uses better-sqlite3 (already a CLI dependency).
 */

import Database from 'better-sqlite3';
import { join } from 'node:path';
import { mkdirSync } from 'node:fs';
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
   */
  constructor(opts = {}) {
    this._dbPath = opts.dbPath || defaultDbPath();
    this._db = new Database(this._dbPath);
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
    return this._searchStmt.all(channel, senderId, `%${query}%`, limit).map(this._deserialize);
  }

  /**
   * Get all recent memories (across all senders).
   * @param {number} [limit=20]
   * @returns {Object[]}
   */
  getAllRecent(limit = 20) {
    return this._allRecentStmt.all(limit).map(this._deserialize);
  }

  /**
   * Delete memories older than the given age.
   * @param {number} maxAgeMs - Max age in milliseconds (default: 30 days)
   * @returns {number} - Number of deleted entries
   */
  prune(maxAgeMs = 30 * 24 * 60 * 60 * 1000) {
    const cutoff = Date.now() - maxAgeMs;
    return this._deleteOldStmt.run(cutoff).changes;
  }

  /**
   * Delete a specific memory by ID.
   * @param {number} id
   * @returns {boolean}
   */
  delete(id) {
    return this._deleteByIdStmt.run(id).changes > 0;
  }

  /**
   * Count total memories.
   * @returns {number}
   */
  count() {
    return this._countStmt.get().cnt;
  }

  /**
   * Close the database.
   */
  close() {
    if (this._db) {
      this._db.close();
      this._db = null;
    }
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
